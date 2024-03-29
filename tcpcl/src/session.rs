// Copyright (C) 2023 Felix Huettner
//
// This file is part of DTRD.
//
// DTRD is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// DTRD is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with this program.  If not, see <https://www.gnu.org/licenses/>.

use std::{
    pin::Pin,
    time::{Duration, Instant},
};

use futures_util::StreamExt;
use log::{debug, error, info, warn};
use openssl::{
    error::ErrorStack,
    ssl::{Ssl, SslAcceptor, SslContext, SslMethod, SslVerifyMode},
    x509::{store::X509StoreBuilder, X509},
};
use tokio::{
    io::{AsyncRead, AsyncWrite, AsyncWriteExt},
    net::TcpStream,
    sync::{mpsc, oneshot},
    time::Interval,
};
use tokio_openssl::SslStream;
use tokio_util::codec::{FramedRead, FramedWrite};
use url::{Host, Url};
use x509_parser::{
    extensions::{GeneralName, ParsedExtension},
    prelude::{FromDer, X509Certificate},
};

use crate::{
    connection_info::ConnectionInfo,
    errors::{ErrorType, Errors, TransferSendErrors},
    transfer::Transfer,
    v4::{
        messages::{self, sess_term::ReasonCode, xfer_segment, Codec, Messages},
        statemachine::StateMachine,
    },
    TLSSettings,
};

pub trait AsyncReadWrite: AsyncRead + AsyncWrite + Send {}
impl<T> AsyncReadWrite for T where T: AsyncRead + AsyncWrite + Send {}

type CustomFramedReader = FramedRead<tokio::io::ReadHalf<Pin<Box<dyn AsyncReadWrite>>>, Codec>;
type CustomFramedWriter = FramedWrite<tokio::io::WriteHalf<Pin<Box<dyn AsyncReadWrite>>>, Codec>;

struct Stream {
    read: CustomFramedReader,
    write: CustomFramedWriter,
    peer_cert: Option<X509>,
}

impl Stream {
    fn from_tcp_stream(ts: TcpStream) -> Self {
        let boxed_stream: Pin<Box<dyn AsyncReadWrite>> = Box::pin(ts);
        let (read, write) = tokio::io::split(boxed_stream);
        Stream {
            read: FramedRead::new(read, Codec::default()),
            write: FramedWrite::new(write, Codec::default()),
            peer_cert: None,
        }
    }

    async fn upgrade_tls(self, ssl: Ssl, is_server: bool) -> Result<Self, ErrorType> {
        let decoder = self.read.decoder().clone(); // need to clone this to keep the state, not relevant for writing since we dont use states there
        let stream = self.read.into_inner().unsplit(self.write.into_inner());
        let mut ssl_stream = SslStream::new(ssl, stream)?;
        if is_server {
            Pin::new(&mut ssl_stream).accept().await?;
        } else {
            Pin::new(&mut ssl_stream).connect().await?;
        }
        let peer_cert = ssl_stream.ssl().peer_certificate();
        let boxed_stream: Pin<Box<dyn AsyncReadWrite>> = Box::pin(ssl_stream);
        let (read, write) = tokio::io::split(boxed_stream);
        Ok(Stream {
            read: FramedRead::new(read, decoder),
            write: FramedWrite::new(write, Codec::default()),
            peer_cert,
        })
    }

    fn get_peer_certificate(&mut self) -> Option<&X509> {
        self.peer_cert.as_ref()
    }

    fn as_split(&mut self) -> (&mut CustomFramedReader, &mut CustomFramedWriter) {
        (&mut self.read, &mut self.write)
    }

    async fn shutdown(self) -> Result<(), std::io::Error> {
        self.write.into_inner().shutdown().await
    }
}

type TransferRequest = (Vec<u8>, oneshot::Sender<Result<(), TransferSendErrors>>);

const STARTUP_IDLE_INTERVAL: u16 = 60;

pub struct TCPCLSession {
    is_server: bool,
    stream: Option<Stream>,
    ssl_context: Option<SslContext>,
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
        let can_tls = tls_settings.is_some();
        let established_channel = oneshot::channel();
        let close_channel = oneshot::channel();
        let receive_channel = mpsc::channel(10);
        let send_channel = mpsc::channel(10);

        let ssl_context = match tls_settings {
            Some(s) => Some(TCPCLSession::make_ssl_context(s)?),
            None => None,
        };
        let peer_url = Url::parse(&format!("tcpcl://{}", stream.peer_addr().unwrap())).unwrap();

        Ok(TCPCLSession {
            is_server: true,
            stream: Some(Stream::from_tcp_stream(stream)),
            ssl_context,
            statemachine: StateMachine::new_passive(node_id, can_tls),
            receiving_transfer: None,
            connection_info: ConnectionInfo {
                peer_endpoint: None,
                peer_url,
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
        url: Url,
        node_id: String,
        tls_settings: Option<TLSSettings>,
    ) -> Result<Self, ErrorType> {
        let addr = url
            .socket_addrs(|| Some(4556))
            .map_err(|_| ErrorType::DnsError)
            .and_then(|mut r| r.pop().ok_or(ErrorType::DnsError))?;
        let stream = TcpStream::connect(addr)
            .await
            .map_err::<ErrorType, _>(|e| e.into())?;
        debug!("Connected to peer at {}", url);
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
            statemachine: StateMachine::new_active(node_id, can_tls),
            receiving_transfer: None,
            connection_info: ConnectionInfo {
                peer_endpoint: None,
                peer_url: url,
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
        self.established_channel
            .1
            .take()
            .expect("May not get a established channel > 1 time")
    }

    pub fn get_close_channel(&mut self) -> oneshot::Sender<()> {
        self.close_channel
            .0
            .take()
            .expect("May not get a close channel > 1 time")
    }

    pub fn get_receive_channel(&mut self) -> mpsc::Receiver<Transfer> {
        self.receive_channel
            .1
            .take()
            .expect("May not get a receive channel > 1 time")
    }

    pub fn get_send_channel(&mut self) -> mpsc::Sender<TransferRequest> {
        self.send_channel.0.clone()
    }

    pub fn get_connection_info(&self) -> ConnectionInfo {
        self.connection_info.clone()
    }

    pub async fn manage_connection(&mut self) -> Result<(), ErrorType> {
        self.last_received_keepalive = Instant::now();

        let mut send_channel_receiver = self
            .send_channel
            .1
            .take()
            .expect("can not manage the connection > 1 time");

        let out = self.drive_statemachine(&mut send_channel_receiver).await;
        if out.is_err() {
            let error = out.unwrap_err();
            warn!("Connection completed with error {:?}", error);
            if self.stream.is_some() {
                let stream = self.stream.take().unwrap();
                if let Err(internal_error) = stream.shutdown().await {
                    warn!("error shuting down the socket: {:?}", internal_error);
                    return Err(error);
                }
            }
            return Err(error);
        } else {
            debug!("Connection has completed");
        }
        Ok(())
    }

    async fn drive_statemachine(
        &mut self,
        scr: &mut mpsc::Receiver<TransferRequest>,
    ) -> Result<(), ErrorType> {
        let mut send_channel_receiver = Some(scr);

        let mut close_channel = self
            .close_channel
            .1
            .take()
            .expect("can not manage the connection > 1 time");

        let mut keepalive_timer: Option<Interval> = Some(tokio::time::interval(
            Duration::from_secs(STARTUP_IDLE_INTERVAL.into()),
        ));

        loop {
            debug!("We are now at statemachine state {:?}", self.statemachine);
            if !self.initialized_tls && self.statemachine.contact_header_done() {
                if self.statemachine.should_use_tls() {
                    let ssl = Ssl::new(self.ssl_context.as_ref().unwrap())?;
                    self.stream = Some(
                        self.stream
                            .take()
                            .unwrap()
                            .upgrade_tls(ssl, self.is_server)
                            .await?,
                    );
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
                debug!("We are done. Closing connection");
                self.stream.take().unwrap().shutdown().await?;
                return Ok(());
            }

            let stream = self.stream.as_mut().unwrap();
            let (read_stream, write_stream) = stream.as_split();

            let stream_interest = self.statemachine.get_interests();

            tokio::select! {
                read_out = async { read_stream.next().await }, if stream_interest.is_readable() => {
                    match read_out {
                        None => {
                            debug!("Connection closed by peer");
                            return Ok(());
                        }
                        Some(message) => {
                            match self.read_message(message).await {
                                Ok(_) => {},
                                Err(e) => {return Err(e)},
                            };
                        }
                    }
                }
                res = async { self.statemachine.send_message(write_stream).await }, if stream_interest.is_writable() => {
                    res?
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
                    if self.statemachine.is_established() && self.last_received_keepalive.elapsed() > Duration::from_secs(self.statemachine.get_keepalive_interval().unwrap_or(STARTUP_IDLE_INTERVAL).into()) * 2 {
                        self.statemachine.close_connection(Some(ReasonCode::IdleTimeout));
                    }
                    if self.initialized_keepalive {
                        self.statemachine.send_keepalive();
                    }
                }
                _ = (&mut close_channel), if !self.statemachine.connection_closing() && self.statemachine.is_established() => {
                    self.statemachine.close_connection(Some(ReasonCode::ResourceExhaustion));
                }
            }
        }
    }

    async fn read_message(
        &mut self,
        message: Result<Messages, messages::Errors>,
    ) -> Result<(), ErrorType> {
        let msg = self.statemachine.decode_message(message);
        match msg {
            Ok(Messages::ContactHeader(h)) => {
                debug!("Got contact header: {:?}", h);
            }
            Ok(Messages::SessInit(s)) => {
                debug!("Got sessinit: {:?}", s);
                if self.statemachine.should_use_tls() {
                    let peer_node_id = s.node_id;
                    let x509 = self.stream.as_mut().unwrap().get_peer_certificate();
                    if validate_peer_certificate(
                        peer_node_id.clone(),
                        &self.connection_info.peer_url,
                        x509,
                    )
                    .is_err()
                    {
                        return Err(Errors::TLSNameMissmatch(peer_node_id).into());
                    }
                }
            }
            Ok(Messages::SessTerm(s)) => {
                debug!("Got sessterm: {:?}", s);
            }
            Ok(Messages::Keepalive(_)) => {
                debug!("Got keepalive");
                self.last_received_keepalive = Instant::now();
            }
            Ok(Messages::XferSegment(x)) => {
                debug!("Got xfer segment {:?}", x);
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
                    debug!("Fully received transfer {}, passing it up", x.transfer_id);
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
            Ok(Messages::XferAck(_)) => {
                //statemachine cares about it
            }
            Ok(Messages::XferRefuse(x)) => {
                warn!("Got xfer refuse, no idea what to do now: {:?}", x);
            }
            Ok(Messages::MsgReject(m)) => {
                info!("Got msg reject: {:?}. Will close the connection now", m);
                return Err(Errors::RemoteRejected.into());
            }
            e @ Err(Errors::MessageError(messages::Errors::InvalidHeader)) => {
                warn!("Header invalid");
                return Err(e.unwrap_err().into());
            }
            e @ Err(Errors::MessageError(messages::Errors::NodeIdInvalid)) => {
                warn!("Remote Node-Id was invalid");
                return Err(e.unwrap_err().into());
            }
            Err(Errors::MessageError(messages::Errors::UnkownCriticalSessionExtension(ext))) => {
                warn!(
                    "Remote send critical session extension {} that we dont know",
                    ext
                );
                return Err(Errors::MessageError(
                    messages::Errors::UnkownCriticalSessionExtension(ext),
                )
                .into());
            }
            Err(Errors::MessageError(messages::Errors::UnkownCriticalTransferExtension(ext))) => {
                warn!(
                    "Remote send critical transfer extension {} that we dont know",
                    ext
                );
                return Err(Errors::MessageError(
                    messages::Errors::UnkownCriticalTransferExtension(ext),
                )
                .into());
            }
            Err(Errors::MessageError(messages::Errors::InvalidMessageType(_))) => {
                warn!("Received a unkown message type");
            }
            Err(Errors::MessageTypeInappropriate(mt)) => {
                warn!(
                    "Remote send message type currently not applicable: {:?}",
                    mt
                );
            }
            Err(Errors::RemoteRejected) => {
                warn!("In the remote rejected state");
            }
            Err(Errors::TLSNameMissmatch(_)) => {
                warn!("In the tls name missmatch state");
            }
            e @ Err(Errors::MessageError(messages::Errors::InvalidACKValue)) => {
                return Err(e.unwrap_err().into());
            }
            e @ Err(Errors::DoesNotSpeakTCPCL) => {
                error!("The remote end does not follow the tcpcl protocl");
                return Err(e.unwrap_err().into());
            }
            e @ Err(Errors::MessageError(messages::Errors::SegmentTooLong)) => {
                warn!("We received a segment longer than our Segment MRU");
                return Err(e.unwrap_err().into());
            }
            e @ Err(Errors::MessageError(messages::Errors::IoError(_))) => {
                warn!("We had some io error {:?}", e);
                return Err(e.unwrap_err().into());
            }
        }
        Ok(())
    }
}

fn validate_peer_certificate(
    peer_node_id: String,
    peer_url: &Url,
    x509: Option<&X509>,
) -> Result<(), ()> {
    match x509 {
        Some(cert) => {
            let cert_bytes = cert.to_der().map_err(|_| ())?;
            let (_, c) = X509Certificate::from_der(&cert_bytes).map_err(|_| ())?;
            for extension in c.extensions() {
                if let ParsedExtension::SubjectAlternativeName(sans) = extension.parsed_extension()
                {
                    for san in &sans.general_names {
                        if let GeneralName::OtherName(oid, value) = san {
                            if oid.to_id_string() == "1.3.6.1.5.5.7.8.11"
                                && &value[4..] == peer_node_id.as_bytes()
                            // we strip of the first 4 bytes as they are the ASN.1 header for a list of one string
                            {
                                debug!("Certificate matched");
                                return Ok(());
                            }
                        }
                    }
                    // If we did not find a matching other name, then try dns names
                    // TODO: make this configurable
                    if let Host::Domain(peer_name) = peer_url.host().unwrap() {
                        for san in &sans.general_names {
                            if let GeneralName::DNSName(name) = san {
                                if name == &peer_name {
                                    debug!("Certificate matched");
                                    return Ok(());
                                }
                            }
                        }
                    }
                }
            }
        }
        None => {
            warn!("We did not get a peer certificate for the tls session.");
        }
    }
    Err(())
}
