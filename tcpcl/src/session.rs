use std::net::SocketAddr;

use log::{debug, info, warn};
use tokio::{
    io::{self, AsyncWriteExt},
    net::TcpStream,
};

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
}

impl TCPCLSession {
    pub async fn new(stream: TcpStream) -> Result<Self, ErrorType> {
        let mut sess = TCPCLSession {
            stream,
            reader: Reader::new(),
            writer: Vec::new(),
            statemachine: StateMachine::new_passive(),
        };

        sess.drive_statemachine().await?;

        return Ok(sess);
    }

    pub async fn connect(socket: SocketAddr) -> Result<Self, ErrorType> {
        let stream = TcpStream::connect(&socket)
            .await
            .map_err::<ErrorType, _>(|e| e.into())?;
        debug!("Connected to peer at {}", socket);
        let mut sess = TCPCLSession {
            stream,
            reader: Reader::new(),
            writer: Vec::new(),
            statemachine: StateMachine::new_active(),
        };

        sess.drive_statemachine().await?;

        return Ok(sess);
    }

    pub async fn close(&mut self, reason: Option<ReasonCode>) -> Result<(), ErrorType> {
        self.statemachine.close_connection(reason);

        self.drive_statemachine().await
    }

    pub async fn wait(&mut self) -> Result<(), ErrorType> {
        self.drive_statemachine().await
    }

    async fn drive_statemachine(&mut self) -> Result<(), ErrorType> {
        loop {
            debug!("We are now at state {:?}", self.statemachine.state);

            let ready = self.stream.ready(self.statemachine.get_interests()).await?;

            if ready.is_readable() {
                match self.reader.read(&mut self.stream).await {
                    Ok(0) => {
                        info!("Connection closed by peer");
                        return Ok(());
                    }
                    Ok(n) => {
                        info!("read {} bytes", n);
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
                                continue;
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
                        info!("wrote {} bytes", n);
                        if self.writer.len() == n {
                            self.writer.clear();
                            self.statemachine.send_complete();
                            info!("Write complete");
                        } else {
                            self.writer.drain(0..n);
                            info!("write incomplete. Trying again");
                        }
                    }
                    Err(_) => {}
                }
            }

            if self.statemachine.should_close() {
                info!("We are done. Closing connection");
                self.stream.shutdown().await?;
                return Ok(());
            }
            if self.statemachine.is_established() {
                info!("Session to peer {} established", self.stream.peer_addr()?);
                return Ok(());
            }
        }
    }
}
