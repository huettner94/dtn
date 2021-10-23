use std::net::SocketAddr;

use log::{debug, info, warn};
use tokio::{
    io::{self, AsyncWriteExt, Interest, Ready},
    net::TcpStream,
    sync::{mpsc, oneshot},
};

use crate::{
    connection_info::ConnectionInfo,
    errors::{ErrorType, Errors},
    transfer::Transfer,
    v4::{
        messages::{sess_term::ReasonCode, xfer_segment, Messages},
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
    receiving_transfer: Option<Transfer>,
    connection_info: Option<ConnectionInfo>,
    established_channel: (
        Option<oneshot::Sender<ConnectionInfo>>,
        Option<oneshot::Receiver<ConnectionInfo>>,
    ),
    close_channel: (Option<oneshot::Sender<()>>, Option<oneshot::Receiver<()>>),
    receive_channel: (mpsc::Sender<Transfer>, Option<mpsc::Receiver<Transfer>>),
    send_channel: (mpsc::Sender<Vec<u8>>, Option<mpsc::Receiver<Vec<u8>>>),
}

impl TCPCLSession {
    pub fn new(stream: TcpStream, node_id: String) -> Self {
        let established_channel = oneshot::channel();
        let close_channel = oneshot::channel();
        let receive_channel = mpsc::channel(10);
        let send_channel = mpsc::channel(10);
        TCPCLSession {
            stream,
            reader: Reader::new(),
            writer: Vec::new(),
            statemachine: StateMachine::new_passive(node_id),
            receiving_transfer: None,
            connection_info: None,
            established_channel: (Some(established_channel.0), Some(established_channel.1)),
            close_channel: (Some(close_channel.0), Some(close_channel.1)),
            receive_channel: (receive_channel.0, Some(receive_channel.1)),
            send_channel: (send_channel.0, Some(send_channel.1)),
        }
    }

    pub async fn connect(socket: SocketAddr, node_id: String) -> Result<Self, ErrorType> {
        let stream = TcpStream::connect(&socket)
            .await
            .map_err::<ErrorType, _>(|e| e.into())?;
        debug!("Connected to peer at {}", socket);
        let established_channel = oneshot::channel();
        let close_channel = oneshot::channel();
        let receive_channel = mpsc::channel(10);
        let send_channel = mpsc::channel(10);
        Ok(TCPCLSession {
            stream,
            reader: Reader::new(),
            writer: Vec::new(),
            statemachine: StateMachine::new_active(node_id),
            receiving_transfer: None,
            connection_info: None,
            established_channel: (Some(established_channel.0), Some(established_channel.1)),
            close_channel: (Some(close_channel.0), Some(close_channel.1)),
            receive_channel: (receive_channel.0, Some(receive_channel.1)),
            send_channel: (send_channel.0, Some(send_channel.1)),
        })
    }

    pub fn get_established_channel(&mut self) -> oneshot::Receiver<ConnectionInfo> {
        return self
            .established_channel
            .1
            .take()
            .expect("May not get a established channel > 1 time");
    }

    pub fn get_close_channel(&mut self) -> oneshot::Sender<()> {
        return self
            .close_channel
            .0
            .take()
            .expect("May not get a close channel > 1 time");
    }

    pub fn get_receive_channel(&mut self) -> mpsc::Receiver<Transfer> {
        return self
            .receive_channel
            .1
            .take()
            .expect("May not get a receive channel > 1 time");
    }

    pub fn get_send_channel(&mut self) -> mpsc::Sender<Vec<u8>> {
        return self.send_channel.0.clone();
    }

    pub fn get_connection_info(&self) -> Option<ConnectionInfo> {
        self.connection_info.clone()
    }

    pub async fn manage_connection(&mut self) {
        let mut close_channel = self
            .close_channel
            .1
            .take()
            .expect("can not manage the connection > 1 time");
        let mut send_channel_receiver = self
            .send_channel
            .1
            .take()
            .expect("can not manage the connection > 1 time");

        loop {
            tokio::select! {
                out = self.drive_statemachine(&mut send_channel_receiver) => {
                    if out.is_err() {
                        warn!("Connection completed with error {:?}", out.unwrap_err());
                    } else {
                        info!("Connection has completed");
                    }
                    break;
                }
                _ = (&mut close_channel), if !self.statemachine.connection_closing() => {
                    self.statemachine.close_connection(Some(ReasonCode::ResourceExhaustion));
                }
            }
        }
    }

    async fn drive_statemachine(
        &mut self,
        scr: &mut mpsc::Receiver<Vec<u8>>,
    ) -> Result<(), ErrorType> {
        let mut send_channel_receiver = Some(scr);
        loop {
            debug!("We are now at statemachine state {:?}", self.statemachine);
            if self.statemachine.is_established() && self.established_channel.0.is_some() {
                let connection_info = ConnectionInfo {
                    peer_endpoint: self.statemachine.get_peer_node_id(),
                };
                self.connection_info = Some(connection_info.clone());
                if let Err(e) = self
                    .established_channel
                    .0
                    .take()
                    .unwrap()
                    .send(connection_info)
                {
                    warn!("Error sending connection info: {:?}", e);
                };
            }

            if self.statemachine.should_close() {
                info!("We are done. Closing connection");
                self.stream.shutdown().await?;
                return Ok(());
            }

            let stream_interest = self.statemachine.get_interests();
            if stream_interest == Interest::READABLE && self.reader.left() > 0 {
                match self.read_message().await {
                    Ok(_) => continue,
                    Err(ErrorType::TCPCLError(Errors::MessageTooShort)) => {}
                    Err(e) => return Err(e),
                }
            }

            tokio::select! {
                ready = self.stream.ready(stream_interest) => {
                    match self.handle_socket_ready(ready?).await {
                        Ok(true) => {return Ok(())},
                        Ok(false) => {},
                        Err(e) => {return Err(e);},
                    };
                }
                transfer = send_channel_receiver.as_mut().unwrap().recv(), if send_channel_receiver.is_some() && self.statemachine.could_send_transfer() => {
                    match transfer {
                        Some(t) => {
                            self.statemachine.send_transfer(t);
                        },
                        None => {send_channel_receiver = None;}
                    }
                }
            }
        }
    }

    async fn handle_socket_ready(&mut self, ready: Ready) -> Result<bool, ErrorType> {
        if ready.is_readable() {
            match self.reader.read(&mut self.stream).await {
                Ok(0) => {
                    info!("Connection closed by peer");
                    return Ok(true);
                }
                Ok(_) => {
                    self.read_message().await?;
                    // We return here as we need to think about our states again
                    return Ok(false);
                }
                Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {}
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
                    return Ok(true);
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
        Ok(false)
    }

    async fn read_message(&mut self) -> Result<(), ErrorType> {
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
                info!("Got xfer segment {:?}", x);
                if self.receiving_transfer.is_some()
                    && x.flags.contains(xfer_segment::MessageFlags::START)
                {
                    warn!(
                        "Remote startet transfer with id {} while {} is still being received",
                        x.transfer_id,
                        self.receiving_transfer.as_ref().unwrap().id
                    );
                    //TODO close connection
                }

                let ack = match &mut self.receiving_transfer {
                    Some(t) => {
                        if t.id == x.transfer_id {
                            t.data.extend_from_slice(&x.data);
                            x.to_xfer_ack(t.data.len() as u64)
                        } else {
                            warn!(
                                "Remote sent transfer with id {} while {} is still being received",
                                x.transfer_id, t.id
                            );
                            //TODO close connection
                            return Ok(());
                        }
                    }
                    None => {
                        if !x.flags.contains(xfer_segment::MessageFlags::START) {
                            warn!("Remote did not sent a start flag for a new transfer. Accepting it anyway");
                        }
                        let a = x.to_xfer_ack(x.data.len() as u64);
                        self.receiving_transfer = Some(Transfer {
                            id: x.transfer_id,
                            data: x.data,
                        });
                        a
                    }
                };

                if x.flags.contains(xfer_segment::MessageFlags::END) {
                    info!("Fully received transfer {}, passing it up", x.transfer_id);
                    if let Err(e) = self
                        .receive_channel
                        .0
                        .send(self.receiving_transfer.take().unwrap())
                        .await
                    {
                        warn!("Error sending transfer to receive channel: {:?}", e);
                        //TODO close connection
                    };
                }
                self.statemachine.send_ack(ack);
            }
            Ok(Messages::XferAck(x)) => {
                debug!(
                    "Got xfer ack, we don't do things as the statemachine cares about that: {:?}",
                    x
                );
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
