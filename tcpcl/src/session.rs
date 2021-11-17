use std::{
    net::SocketAddr,
    pin::Pin,
    task::{Context, Poll},
    time::{Duration, Instant},
};

use log::{debug, info, warn};
use openssl::ssl::{Ssl, SslContext};
use tokio::{
    io::{self, AsyncRead, AsyncWrite, AsyncWriteExt, Interest},
    net::TcpStream,
    sync::{mpsc, oneshot},
    time::Interval,
};
use tokio_openssl::SslStream;

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

struct Stream {
    tcp_read: Option<tokio::io::ReadHalf<TcpStream>>,
    tcp_write: Option<tokio::io::WriteHalf<TcpStream>>,
    tls_read: Option<tokio::io::ReadHalf<SslStream<TcpStream>>>,
    tls_write: Option<tokio::io::WriteHalf<SslStream<TcpStream>>>,
}

impl Stream {
    fn from_tcp_stream(ts: TcpStream) -> Self {
        let (tcp_read, tcp_write) = tokio::io::split(ts);
        Stream {
            tcp_read: Some(tcp_read),
            tcp_write: Some(tcp_write),
            tls_read: None,
            tls_write: None,
        }
    }

    fn from_tls_stream(ssl_stream: SslStream<TcpStream>) -> Self {
        let (tls_read, tls_write) = tokio::io::split(ssl_stream);
        Stream {
            tcp_read: None,
            tcp_write: None,
            tls_read: Some(tls_read),
            tls_write: Some(tls_write),
        }
    }

    fn get_tcp_stream(self) -> TcpStream {
        if self.tcp_read.is_none() {
            panic!("Cant get tcp stream if we dont have one");
        }
        self.tcp_read.unwrap().unsplit(self.tcp_write.unwrap())
    }

    fn as_split(
        &mut self,
    ) -> (
        Box<dyn AsyncRead + Unpin + Send + '_>,
        Box<dyn AsyncWrite + Unpin + Send + '_>,
    ) {
        if self.tcp_write.is_some() {
            (
                Box::new(self.tcp_read.as_mut().unwrap()),
                Box::new(self.tcp_write.as_mut().unwrap()),
            )
        } else {
            (
                Box::new(self.tls_read.as_mut().unwrap()),
                Box::new(self.tls_write.as_mut().unwrap()),
            )
        }
    }
}
impl AsyncWrite for Stream {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<Result<usize, std::io::Error>> {
        if self.tcp_write.is_some() {
            Pin::new(self.tcp_write.as_mut().unwrap()).poll_write(cx, buf)
        } else {
            Pin::new(self.tls_write.as_mut().unwrap()).poll_write(cx, buf)
        }
    }

    fn poll_flush(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<(), std::io::Error>> {
        if self.tcp_write.is_some() {
            Pin::new(self.tcp_write.as_mut().unwrap()).poll_flush(cx)
        } else {
            Pin::new(self.tls_write.as_mut().unwrap()).poll_flush(cx)
        }
    }

    fn poll_shutdown(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<(), std::io::Error>> {
        if self.tcp_write.is_some() {
            Pin::new(self.tcp_write.as_mut().unwrap()).poll_shutdown(cx)
        } else {
            Pin::new(self.tls_write.as_mut().unwrap()).poll_shutdown(cx)
        }
    }
}

const STARTUP_IDLE_INTERVAL: u16 = 60;

pub struct TCPCLSession {
    is_server: bool,
    stream: Option<Stream>,
    ssl_context: Option<SslContext>,
    reader: Reader,
    writer: Vec<u8>,
    statemachine: StateMachine,
    receiving_transfer: Option<Transfer>,
    connection_info: ConnectionInfo,
    established_channel: (
        Option<oneshot::Sender<ConnectionInfo>>,
        Option<oneshot::Receiver<ConnectionInfo>>,
    ),
    close_channel: (Option<oneshot::Sender<()>>, Option<oneshot::Receiver<()>>),
    receive_channel: (mpsc::Sender<Transfer>, Option<mpsc::Receiver<Transfer>>),
    send_channel: (mpsc::Sender<Vec<u8>>, Option<mpsc::Receiver<Vec<u8>>>),
    last_received_keepalive: Instant,
}

impl TCPCLSession {
    pub fn new(
        stream: TcpStream,
        node_id: String,
        ssl_context: Option<SslContext>,
    ) -> Result<Self, std::io::Error> {
        let peer_addr = stream.peer_addr()?;
        let can_tls = ssl_context.is_some();
        let established_channel = oneshot::channel();
        let close_channel = oneshot::channel();
        let receive_channel = mpsc::channel(10);
        let send_channel = mpsc::channel(10);
        Ok(TCPCLSession {
            is_server: true,
            stream: Some(Stream::from_tcp_stream(stream)),
            ssl_context,
            reader: Reader::new(),
            writer: Vec::new(),
            statemachine: StateMachine::new_passive(node_id, can_tls),
            receiving_transfer: None,
            connection_info: ConnectionInfo {
                peer_endpoint: None,
                peer_address: peer_addr,
            },
            established_channel: (Some(established_channel.0), Some(established_channel.1)),
            close_channel: (Some(close_channel.0), Some(close_channel.1)),
            receive_channel: (receive_channel.0, Some(receive_channel.1)),
            send_channel: (send_channel.0, Some(send_channel.1)),
            last_received_keepalive: Instant::now(),
        })
    }

    pub async fn connect(
        socket: SocketAddr,
        node_id: String,
        ssl_context: Option<SslContext>,
    ) -> Result<Self, ErrorType> {
        let stream = TcpStream::connect(&socket)
            .await
            .map_err::<ErrorType, _>(|e| e.into())?;
        debug!("Connected to peer at {}", socket);
        let can_tls = ssl_context.is_some();
        let established_channel = oneshot::channel();
        let close_channel = oneshot::channel();
        let receive_channel = mpsc::channel(10);
        let send_channel = mpsc::channel(10);
        Ok(TCPCLSession {
            is_server: false,
            stream: Some(Stream::from_tcp_stream(stream)),
            ssl_context,
            reader: Reader::new(),
            writer: Vec::new(),
            statemachine: StateMachine::new_active(node_id, can_tls),
            receiving_transfer: None,
            connection_info: ConnectionInfo {
                peer_endpoint: None,
                peer_address: socket,
            },
            established_channel: (Some(established_channel.0), Some(established_channel.1)),
            close_channel: (Some(close_channel.0), Some(close_channel.1)),
            receive_channel: (receive_channel.0, Some(receive_channel.1)),
            send_channel: (send_channel.0, Some(send_channel.1)),
            last_received_keepalive: Instant::now(),
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

    pub fn get_connection_info(&self) -> ConnectionInfo {
        self.connection_info.clone()
    }

    pub async fn manage_connection(&mut self) {
        self.last_received_keepalive = Instant::now();

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
                    if let Err(e) = self.stream.as_mut().unwrap().shutdown().await {
                        warn!("error shuting down the socket: {:?}", e);
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
        let mut keepalive_timer: Option<Interval> = Some(tokio::time::interval(
            Duration::from_secs(STARTUP_IDLE_INTERVAL.into()),
        ));
        let mut initialized_keepalive = false;
        let mut initialized_tls = false;

        loop {
            debug!("We are now at statemachine state {:?}", self.statemachine);
            if !initialized_tls && self.statemachine.contact_header_done() {
                if self.statemachine.should_use_tls() {
                    let stream = self.stream.take().unwrap().get_tcp_stream();
                    let ssl = Ssl::new(self.ssl_context.as_ref().unwrap()).unwrap();
                    let mut ssl_stream = SslStream::new(ssl, stream).unwrap();
                    if self.is_server {
                        Pin::new(&mut ssl_stream).accept().await.unwrap();
                    } else {
                        Pin::new(&mut ssl_stream).connect().await.unwrap();
                    }
                    self.stream = Some(Stream::from_tls_stream(ssl_stream));
                }
                initialized_tls = true;
            }

            if self.statemachine.is_established() && self.established_channel.0.is_some() {
                self.connection_info.peer_endpoint = Some(self.statemachine.get_peer_node_id());

                if let Err(e) = self
                    .established_channel
                    .0
                    .take()
                    .unwrap()
                    .send(self.connection_info.clone())
                {
                    warn!("Error sending connection info: {:?}", e);
                };
            }

            if self.statemachine.is_established() && !initialized_keepalive {
                match self.statemachine.get_keepalive_interval() {
                    Some(interval) => {
                        keepalive_timer =
                            Some(tokio::time::interval(Duration::from_secs(interval.into())));
                        keepalive_timer
                            .as_mut()
                            .unwrap()
                            .set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
                    }
                    None => {
                        keepalive_timer = None;
                    }
                }
                initialized_keepalive = true;
            }

            if self.statemachine.should_close() {
                info!("We are done. Closing connection");
                self.stream.as_mut().unwrap().shutdown().await?;
                return Ok(());
            }

            let mut stream_interest = self.statemachine.get_interests();
            if !self.writer.is_empty() {
                stream_interest |= Interest::WRITABLE;
            }
            if stream_interest == Interest::READABLE && self.reader.left() > 0 {
                match self.read_message().await {
                    Ok(_) => continue,
                    Err(ErrorType::TCPCLError(Errors::MessageTooShort)) => {}
                    Err(e) => return Err(e),
                }
            }

            if self.writer.is_empty() && stream_interest.is_writable() {
                self.statemachine.send_message(&mut self.writer);
            }

            let stream = self.stream.as_mut().unwrap();
            let (mut read_stream, mut write_stream) = stream.as_split();

            tokio::select! {
                read_out = async { self.reader.read(&mut read_stream).await }, if stream_interest.is_readable() => {
                    match read_out {
                        Ok(0) => {
                            info!("Connection closed by peer");
                            return Ok(());
                        }
                        Ok(_) => {
                            drop(read_stream);
                            drop(write_stream);
                            self.read_message().await?;
                        }
                        Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {}
                        Err(e) => {
                            return Err(e.into());
                        }
                    }
                }
                write_out = async { write_stream.write(&self.writer).await }, if stream_interest.is_writable() => {
                    match write_out {
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
                transfer = async { send_channel_receiver.as_mut().unwrap().recv().await }, if send_channel_receiver.is_some() && self.statemachine.could_send_transfer() => {
                    match transfer {
                        Some(t) => {
                            self.statemachine.send_transfer(t);
                        },
                        None => {send_channel_receiver = None;}
                    }
                }
                _ = async { keepalive_timer.as_mut().unwrap().tick().await }, if keepalive_timer.is_some() => {
                    if self.statemachine.is_established() {
                        if self.last_received_keepalive.elapsed() >
                                Duration::from_secs(self.statemachine.get_keepalive_interval().or(Some(STARTUP_IDLE_INTERVAL)).unwrap().into()) * 2 {
                            self.statemachine.close_connection(Some(ReasonCode::IdleTimeout));
                        }
                    }
                    if initialized_keepalive {
                        self.statemachine.send_keepalive();
                    }
                }
            }
        }
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
                debug!("Got keepalive");
                self.last_received_keepalive = Instant::now();
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
                info!("Got msg reject: {:?}. Will close the connection now", m);
                return Err(Errors::RemoteRejected.into());
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
            Err(Errors::UnkownMessageType) => {
                warn!("Received a unkown message type");
            }
            Err(Errors::MessageTypeInappropriate) => {
                warn!("Remote send message type currently not applicable");
            }
            Err(Errors::RemoteRejected) => {
                warn!("In the remote rejected state");
            }
        }
        Ok(())
    }
}
