use std::net::SocketAddr;

use log::{debug, info, warn};
use tokio::{
    io::{self, AsyncWriteExt, Interest},
    net::TcpStream,
    sync::oneshot,
};

use crate::{
    errors::{ErrorType, Errors},
    v4::{
        messages::{sess_term::ReasonCode, Messages},
        reader::Reader,
        statemachine::StateMachine,
    },
};

#[derive(Debug)]
pub struct TCPCLSession {
    stream: TcpStream,
    reader: Reader,
    writer: Vec<u8>,
    statemachine: StateMachine,
    close_channel: (
        Option<oneshot::Sender<ReasonCode>>,
        Option<oneshot::Receiver<ReasonCode>>,
    ),
}

impl TCPCLSession {
    pub fn new(stream: TcpStream) -> Self {
        let close_channel = oneshot::channel();
        TCPCLSession {
            stream,
            reader: Reader::new(),
            writer: Vec::new(),
            statemachine: StateMachine::new_passive(),
            close_channel: (Some(close_channel.0), Some(close_channel.1)),
        }
    }

    pub async fn connect(socket: SocketAddr) -> Result<Self, ErrorType> {
        let stream = TcpStream::connect(&socket)
            .await
            .map_err::<ErrorType, _>(|e| e.into())?;
        debug!("Connected to peer at {}", socket);
        let close_channel = oneshot::channel();
        Ok(TCPCLSession {
            stream,
            reader: Reader::new(),
            writer: Vec::new(),
            statemachine: StateMachine::new_active(),
            close_channel: (Some(close_channel.0), Some(close_channel.1)),
        })
    }

    pub fn get_close_channel(&mut self) -> oneshot::Sender<ReasonCode> {
        return self
            .close_channel
            .0
            .take()
            .expect("May not get a close channel > 1 time");
    }

    pub async fn manage_connection(mut self) {
        let mut close_channel = self
            .close_channel
            .1
            .take()
            .expect("can not manage the connection > 1 time");
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
                reason = (&mut close_channel), if !self.statemachine.connection_closing() => {
                    self.statemachine.close_connection(Some(reason.unwrap_or(ReasonCode::Unkown)));
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
            Ok(Messages::Keepalive(_)) => {
                info!("Got keepalive");
            }
            Ok(Messages::XferSegment(x)) => {
                info!("Got xfer segment, no idea what to do now: {:?}", x);
            }
            Ok(Messages::XferAck(x)) => {
                info!("Got xfer ack, no idea what to do now: {:?}", x);
            }
            Ok(Messages::XferRefuse(x)) => {
                info!("Got xfer refuse, no idea what to do now: {:?}", x);
            }
            Ok(Messages::MsgReject(m)) => {
                info!("Got msg reject, no idea what to do now: {:?}", m);
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
            Err(Errors::UnkownCriticalTransferExtension(ext)) => {
                warn!(
                    "Remote send critical transfer extension {} that we dont know",
                    ext
                );
                return Err(Errors::UnkownCriticalTransferExtension(ext).into());
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
