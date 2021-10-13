use std::net::SocketAddr;

use log::{debug, info, warn};
use tokio::{
    io::{self, AsyncWriteExt, Interest},
    net::TcpStream,
};
use tokio_util::sync::CancellationToken;

use crate::{
    errors::{ErrorType, Errors},
    messages::{reader::Reader, sess_term::ReasonCode, statemachine::StateMachine, Messages},
};

#[derive(Debug)]
pub struct TCPCLSession {
    stream: TcpStream,
    reader: Reader,
    writer: Vec<u8>,
    statemachine: StateMachine,
    cancellationtoken: CancellationToken,
}

impl TCPCLSession {
    pub fn new(stream: TcpStream) -> Self {
        TCPCLSession {
            stream,
            reader: Reader::new(),
            writer: Vec::new(),
            statemachine: StateMachine::new_passive(),
            cancellationtoken: CancellationToken::new(),
        }
    }

    pub async fn connect(socket: SocketAddr) -> Result<Self, ErrorType> {
        let stream = TcpStream::connect(&socket)
            .await
            .map_err::<ErrorType, _>(|e| e.into())?;
        debug!("Connected to peer at {}", socket);
        Ok(TCPCLSession {
            stream,
            reader: Reader::new(),
            writer: Vec::new(),
            statemachine: StateMachine::new_active(),
            cancellationtoken: CancellationToken::new(),
        })
    }

    pub fn get_cancellation_token(&self) -> CancellationToken {
        self.cancellationtoken.clone()
    }

    pub async fn manage_connection(mut self) {
        let canceltoken = self.cancellationtoken.clone();
        loop {
            tokio::select! {
                out = self.drive_statemachine() => {
                    if out.is_err() {
                        warn!("Connection completed with error {:?}", out.unwrap_err());
                    } else {
                        info!("Connection has completed");
                    }
                    break;
                }
                _ = canceltoken.cancelled(), if !self.statemachine.connection_closing() => {
                    self.statemachine.close_connection(None);
                }
            }
        }
    }

    async fn drive_statemachine(&mut self) -> Result<(), ErrorType> {
        loop {
            debug!("We are now at state {:?}", self.statemachine.state);

            if self.statemachine.should_close() {
                info!("We are done. Closing connection");
                self.stream.shutdown().await?;
                return Ok(());
            }

            let stream_interest = self.statemachine.get_interests();
            if stream_interest == Interest::READABLE && self.reader.left() > 0 {
                match self.read_message() {
                    Ok(_) => continue,
                    Err(ErrorType::TCPCLError(Errors::MessageTooShort)) => {}
                    Err(e) => return Err(e),
                }
            }
            let ready = self.stream.ready(stream_interest).await?;

            if ready.is_readable() {
                match self.reader.read(&mut self.stream).await {
                    Ok(0) => {
                        info!("Connection closed by peer");
                        return Ok(());
                    }
                    Ok(_) => {
                        self.read_message()?;
                    }
                    Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                        continue;
                    }
                    Err(e) => {
                        return Err(e.into());
                    }
                }
            }

            if ready.is_writable() {
                if self.writer.is_empty() {
                    self.statemachine.send_message(&mut self.writer);
                }
                match self.stream.write(&self.writer).await {
                    Ok(0) => {
                        info!("Connection closed");
                        return Ok(());
                    }
                    Ok(n) => {
                        if self.writer.len() == n {
                            self.writer.clear();
                            self.statemachine.send_complete();
                        } else {
                            self.writer.drain(0..n);
                            debug!("write incomplete. Trying again");
                        }
                    }
                    Err(_) => {}
                }
            }
        }
    }

    fn read_message(&mut self) -> Result<(), ErrorType> {
        let msg = self.statemachine.decode_message(&mut self.reader);
        match msg {
            Ok(Messages::ContactHeader(h)) => {
                info!("Got contact header: {:?}", h);
            }
            Ok(Messages::SessInit(s)) => {
                info!("Got sessinit: {:?}", s);
            }
            Ok(Messages::SessTerm(s)) => {
                info!("Got sessterm: {:?}", s);
            }
            Err(Errors::MessageTooShort) => {
                debug!("Message was too short, retrying later");
            }
            e @ Err(Errors::InvalidHeader) => {
                warn!("Header invalid");
                return Err(e.unwrap_err().into());
            }
            e @ Err(Errors::NodeIdInvalid) => {
                warn!("Remote Node-Id was invalid");
                return Err(e.unwrap_err().into());
            }
            Err(Errors::UnkownCriticalSessionExtension(ext)) => {
                warn!(
                    "Remote send critical session extension {} that we dont know",
                    ext
                );
                return Err(Errors::UnkownCriticalSessionExtension(ext).into());
            }
            e @ Err(Errors::UnkownMessageType) => {
                warn!("Received a unkown message type");
                return Err(e.unwrap_err().into());
            }
            e @ Err(Errors::MessageTypeInappropriate) => {
                warn!("Remote send message type currently not applicable");
                return Err(e.unwrap_err().into());
            }
        }
        Ok(())
    }
}
