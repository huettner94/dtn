use std::{
    net::SocketAddr,
    pin::Pin,
    time::{Duration, Instant},
};

use log::{debug, error, info, warn};
use openssl::{
    error::ErrorStack,
    ssl::{Ssl, SslAcceptor, SslContext, SslMethod, SslVerifyMode},
    x509::{store::X509StoreBuilder, X509},
};
use tokio::{
    io::{self, AsyncRead, AsyncWrite, AsyncWriteExt, Interest},
    net::TcpStream,
    sync::{mpsc, oneshot},
    time::Interval,
};
use tokio_openssl::SslStream;
use x509_parser::{
    extensions::{GeneralName, ParsedExtension},
    prelude::X509Certificate,
    traits::FromDer,
};

use crate::{
    connection_info::ConnectionInfo,
    errors::{ErrorType, Errors, TransferSendErrors},
    transfer::Transfer,
    v4::{
        messages::{sess_term::ReasonCode, xfer_segment, Messages},
        reader::Reader,
        statemachine::StateMachine,
    },
    TLSSettings,
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

    fn get_peer_certificate(&mut self) -> Option<X509> {
        if self.tls_read.is_none() {
            panic!("Cant get tcp stream if we dont have one");
        }

        // Need to do this dance here as we cant get the ssl details otherwise
        let tls = self
            .tls_read
            .take()
            .unwrap()
            .unsplit(self.tls_write.take().unwrap());
        let x509 = tls.ssl().peer_certificate();

        let (tls_read, tls_write) = tokio::io::split(tls);
        self.tls_read = Some(tls_read);
        self.tls_write = Some(tls_write);

        x509
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

    async fn shutdown(&mut self) -> Result<(), std::io::Error> {
        if self.tcp_write.is_some() {
            self.tcp_write.as_mut().unwrap().shutdown().await
        } else {
            self.tls_write.as_mut().unwrap().shutdown().await
        }
    }
}

type TransferRequest = (Vec<u8>, oneshot::Sender<Result<(), TransferSendErrors>>);

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
    send_channel: (
        mpsc::Sender<TransferRequest>,
        Option<mpsc::Receiver<TransferRequest>>,
    ),
    last_received_keepalive: Instant,

    initialized_keepalive: bool,
    initialized_tls: bool,

    transfer_result_sender: Option<oneshot::Sender<Result<(), TransferSendErrors>>>,
}

impl TCPCLSession {
    fn make_ssl_context(tls_settings: TLSSettings) -> Result<SslContext, ErrorStack> {
        let mut x509_store_builder = X509StoreBuilder::new()?;
        for ca_cert in tls_settings.trusted_certs {
            x509_store_builder.add_cert(ca_cert)?;
        }
        let mut ssl_context_builder = SslAcceptor::mozilla_modern_v5(SslMethod::tls())?;
        ssl_context_builder.set_cert_store(x509_store_builder.build());
        ssl_context_builder.set_private_key(&tls_settings.private_key)?;
        ssl_context_builder.set_certificate(&tls_settings.certificate)?;
        ssl_context_builder.check_private_key()?;
        ssl_context_builder.set_verify(SslVerifyMode::PEER | SslVerifyMode::FAIL_IF_NO_PEER_CERT);
        Ok(ssl_context_builder.build().into_context())
    }

    pub fn new(
        stream: TcpStream,
        node_id: String,
        tls_settings: Option<TLSSettings>,
    ) -> Result<Self, std::io::Error> {
        let peer_addr = stream.peer_addr()?;
        let can_tls = tls_settings.is_some();
        let established_channel = oneshot::channel();
        let close_channel = oneshot::channel();
        let receive_channel = mpsc::channel(10);
        let send_channel = mpsc::channel(10);

        let ssl_context = match tls_settings {
            Some(s) => Some(TCPCLSession::make_ssl_context(s)?),
            None => None,
        };

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
                max_bundle_size: None,
            },
            established_channel: (Some(established_channel.0), Some(established_channel.1)),
            close_channel: (Some(close_channel.0), Some(close_channel.1)),
            receive_channel: (receive_channel.0, Some(receive_channel.1)),
            send_channel: (send_channel.0, Some(send_channel.1)),
            last_received_keepalive: Instant::now(),
            initialized_keepalive: false,
            initialized_tls: false,
            transfer_result_sender: None,
        })
    }

    pub async fn connect(
        socket: SocketAddr,
        node_id: String,
        tls_settings: Option<TLSSettings>,
    ) -> Result<Self, ErrorType> {
        let stream = TcpStream::connect(&socket)
            .await
            .map_err::<ErrorType, _>(|e| e.into())?;
        debug!("Connected to peer at {}", socket);
        let can_tls = tls_settings.is_some();
        let established_channel = oneshot::channel();
        let close_channel = oneshot::channel();
        let receive_channel = mpsc::channel(10);
        let send_channel = mpsc::channel(10);

        let ssl_context = match tls_settings {
            Some(s) => Some(TCPCLSession::make_ssl_context(s)?),
            None => None,
        };

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
                max_bundle_size: None,
            },
            established_channel: (Some(established_channel.0), Some(established_channel.1)),
            close_channel: (Some(close_channel.0), Some(close_channel.1)),
            receive_channel: (receive_channel.0, Some(receive_channel.1)),
            send_channel: (send_channel.0, Some(send_channel.1)),
            last_received_keepalive: Instant::now(),
            initialized_keepalive: false,
            initialized_tls: false,
            transfer_result_sender: None,
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

    pub fn get_send_channel(&mut self) -> mpsc::Sender<TransferRequest> {
        return self.send_channel.0.clone();
    }

    pub fn get_connection_info(&self) -> ConnectionInfo {
        self.connection_info.clone()
    }

    pub async fn manage_connection(&mut self) -> Result<(), ErrorType> {
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
                        if let Err(e) = self.stream.as_mut().unwrap().shutdown().await {
                            warn!("error shuting down the socket: {:?} during handling of error {:?}", e, out.unwrap_err());
                            return Err(e.into());
                        }
                        let e = out.unwrap_err();
                        warn!("Connection completed with error {:?}", e);
                        return Err(e);
                    } else {
                        info!("Connection has completed");
                    }
                    return Ok(());
                }
                _ = (&mut close_channel), if !self.statemachine.connection_closing() => {
                    self.statemachine.close_connection(Some(ReasonCode::ResourceExhaustion));
                }
            }
        }
    }

    async fn drive_statemachine(
        &mut self,
        scr: &mut mpsc::Receiver<TransferRequest>,
    ) -> Result<(), ErrorType> {
        let mut send_channel_receiver = Some(scr);
        let mut keepalive_timer: Option<Interval> = Some(tokio::time::interval(
            Duration::from_secs(STARTUP_IDLE_INTERVAL.into()),
        ));

        loop {
            debug!("We are now at statemachine state {:?}", self.statemachine);
            if !self.initialized_tls && self.statemachine.contact_header_done() {
                if self.statemachine.should_use_tls() {
                    let stream = self.stream.take().unwrap().get_tcp_stream();
                    let ssl = Ssl::new(self.ssl_context.as_ref().unwrap())?;
                    let mut ssl_stream = SslStream::new(ssl, stream)?;
                    if self.is_server {
                        Pin::new(&mut ssl_stream).accept().await?;
                    } else {
                        Pin::new(&mut ssl_stream).connect().await?;
                    }
                    self.stream = Some(Stream::from_tls_stream(ssl_stream));
                }
                self.initialized_tls = true;
            }

            if self.statemachine.is_established() && self.established_channel.0.is_some() {
                self.connection_info.peer_endpoint = Some(self.statemachine.get_peer_node_id());
                self.connection_info.max_bundle_size = Some(self.statemachine.get_peer_mru());

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

            if !self.initialized_keepalive && self.statemachine.is_established() {
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
                self.initialized_keepalive = true;
            }

            if self.statemachine.could_send_transfer() && self.transfer_result_sender.is_some() {
                if let Err(e) = self.transfer_result_sender.take().unwrap().send(Ok(())) {
                    error!("Error sending error to bundle sender {:?}", e);
                }
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
                        Some((bundle_data, result_sender)) => {
                            if let Err(transfer_err) = self.statemachine.send_transfer(bundle_data) {
                                if let Err(e) = result_sender.send(Err(transfer_err)) {
                                    error!("Error sending error to bundle sender {:?}", e);
                                }
                            } else {
                                self.transfer_result_sender = Some(result_sender);
                            }
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
                    if self.initialized_keepalive {
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
                if self.statemachine.should_use_tls() {
                    let peer_node_id = s.node_id;
                    let x509 = self.stream.as_mut().unwrap().get_peer_certificate();
                    if validate_peer_certificate(peer_node_id.clone(), x509).is_err() {
                        return Err(Errors::TLSNameMissmatch(peer_node_id).into());
                    }
                }
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
            Err(Errors::TLSNameMissmatch(_)) => {
                warn!("In the tls name missmatch state");
            }
        }
        Ok(())
    }
}

fn validate_peer_certificate(peer_node_id: String, x509: Option<X509>) -> Result<(), ()> {
    match x509 {
        Some(cert) => {
            let cert_bytes = cert.to_der().map_err(|_| ())?;
            let (_, c) = X509Certificate::from_der(&cert_bytes).map_err(|_| ())?;
            for extension in c.extensions() {
                match extension.parsed_extension() {
                    ParsedExtension::SubjectAlternativeName(sans) => {
                        for san in &sans.general_names {
                            match san {
                                GeneralName::OtherName(oid, value) => {
                                    if oid.to_id_string() == "1.3.6.1.5.5.7.8.11"
                                        && &value[4..] == peer_node_id.as_bytes()
                                    // we strip of the first 4 bytes as they are the ASN.1 header for a list of one string
                                    {
                                        return Ok(());
                                    }
                                }
                                _ => {}
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
        None => {
            warn!("We did not get a peer certificate for the tls session.");
        }
    }
    return Err(());
}
