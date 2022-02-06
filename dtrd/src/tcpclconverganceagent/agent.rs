use std::{collections::HashMap, net::SocketAddr};

use async_trait::async_trait;

use bp7::endpoint::Endpoint;
use log::{error, info, warn};
use openssl::{pkey::PKey, x509::X509};
use tcpcl::{errors::TransferSendErrors, session::TCPCLSession, TLSSettings};
use tokio::{
    fs::File,
    io::AsyncReadExt,
    net::{TcpListener, TcpStream},
    sync::{mpsc, oneshot},
    task::JoinHandle,
};

use crate::{
    common::{settings::Settings, shutdown::Shutdown},
    converganceagent::messages::{AgentForwardBundle, ConverganceAgentRequest},
};

use super::messages::TCPCLAgentRequest;

pub struct Daemon {
    settings: Settings,
    tls_settings: Option<TLSSettings>,
    channel_receiver: Option<mpsc::Receiver<TCPCLAgentRequest>>,
    convergance_agent_sender: Option<mpsc::Sender<ConverganceAgentRequest>>,
    tcpcl_sessions: Vec<JoinHandle<()>>,
    close_channels: HashMap<SocketAddr, oneshot::Sender<()>>,
}

#[async_trait]
impl crate::common::agent::Daemon for Daemon {
    type MessageType = TCPCLAgentRequest;

    async fn new(settings: &Settings) -> Self {
        let tls_settings = match Daemon::load_tls_settings(settings).await {
            Ok(tls) => tls,
            Err(e) => {
                error!(
                    "Error loading tls settings: {:?}. Continuing without tls support",
                    e
                );
                None
            }
        };

        Daemon {
            settings: settings.clone(),
            tls_settings,
            channel_receiver: None,
            convergance_agent_sender: None,
            tcpcl_sessions: Vec::new(),
            close_channels: HashMap::new(),
        }
    }

    fn get_agent_name(&self) -> &'static str {
        "TCPCL Agent"
    }

    fn get_channel_receiver(&mut self) -> Option<mpsc::Receiver<Self::MessageType>> {
        self.channel_receiver.take()
    }

    async fn main_loop(
        &mut self,
        shutdown: &mut Shutdown,
        receiver: &mut mpsc::Receiver<Self::MessageType>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let socket: SocketAddr = self.settings.tcpcl_listen_address.parse().unwrap();

        info!("Server listening on {}", socket);

        let listener = TcpListener::bind(&socket).await?;
        info!("Socket open, waiting for connection");
        while !shutdown.is_shutdown() {
            tokio::select! {
                res = listener.accept() => {
                    if self.handle_accept(res).await {
                        warn!("we are unable to process more incoming connections. stopping tcpcl agent.");
                        break;
                    }
                }
                received = receiver.recv() => {
                    if let Some(msg) = received {
                        self.handle_message(msg).await;
                    } else {
                        info!("TCPCL Agent can no longer receive messages. Exiting");
                        break;
                    }
                }
                _ = shutdown.recv() => {
                    info!("TCPCL agent received shutdown");
                    receiver.close();
                    info!("{} will not allow more requests to be sent", self.get_agent_name());
                }
            }
        }

        info!("Closing the incoming tcp listener");
        drop(listener);

        Ok(())
    }

    async fn on_shutdown(&mut self) {
        // We are explicitly not handling the message receiver here as we cant use it anymore anyway.

        info!("Closing all tcpcl sessions");
        for close_channel in self.close_channels.drain() {
            match close_channel.1.send(()) {
                _ => {}
            }
        }
        for jh in self.tcpcl_sessions.drain(..) {
            match jh.await {
                Ok(_) => {}
                Err(e) => {
                    warn!("Session failed with error: {}", e)
                }
            }
        }
    }

    async fn handle_message(&mut self, msg: TCPCLAgentRequest) {
        match msg {
            TCPCLAgentRequest::ConnectRemote { socket } => {
                self.connect_remote(socket).await;
            }
            TCPCLAgentRequest::DisonnectRemote { socket } => self.disconnect_remote(socket).await,
        }
    }
}

impl Daemon {
    async fn load_tls_settings(settings: &Settings) -> Result<Option<TLSSettings>, std::io::Error> {
        if settings.tcpcl_certificate_path.is_some()
            && settings.tcpcl_key_path.is_some()
            && settings.tcpcl_trusted_certs_path.is_some()
        {
            let mut certificate_file =
                File::open(settings.tcpcl_certificate_path.as_ref().unwrap()).await?;
            let mut certificate_data = Vec::new();
            certificate_file.read_to_end(&mut certificate_data).await?;
            let certificate = X509::from_der(&certificate_data)?;

            let mut key_file = File::open(settings.tcpcl_key_path.as_ref().unwrap()).await?;
            let mut key_data = Vec::new();
            key_file.read_to_end(&mut key_data).await?;
            let key = PKey::private_key_from_der(&key_data)?;

            let mut trusted_file =
                File::open(settings.tcpcl_trusted_certs_path.as_ref().unwrap()).await?;
            let mut trusted_data = Vec::new();
            trusted_file.read_to_end(&mut trusted_data).await?;
            let trusted = X509::from_der(&trusted_data)?;
            info!("Starting TCPCL agent with TLS Support");
            return Ok(Some(TLSSettings::new(key, certificate, vec![trusted])));
        }
        info!("Starting TCPCL agent without TLS Support");
        Ok(None)
    }

    pub fn init_channels(
        &mut self,
        convergance_agent_sender: mpsc::Sender<ConverganceAgentRequest>,
    ) -> mpsc::Sender<TCPCLAgentRequest> {
        self.convergance_agent_sender = Some(convergance_agent_sender);
        let (channel_sender, channel_receiver) = mpsc::channel::<TCPCLAgentRequest>(1);
        self.channel_receiver = Some(channel_receiver);
        return channel_sender;
    }

    async fn connect_remote(&mut self, socket: SocketAddr) {
        match TCPCLSession::connect(
            socket,
            self.settings.my_node_id.clone(),
            self.tls_settings.clone(),
        )
        .await
        {
            Ok(sess) => self.process_socket(sess).await,
            Err(e) => {
                error!("Error connecting to requested remote {}: {:?}", socket, e);
                if let Err(e) = self
                    .convergance_agent_sender
                    .as_ref()
                    .unwrap()
                    .send(ConverganceAgentRequest::CLUnregisterNode {
                        node: None,
                        url: format!("tcpcl://{}", &socket),
                    })
                    .await
                {
                    error!("Error sending message to convergance agent: {:?}", e);
                }
            }
        };
    }

    async fn disconnect_remote(&mut self, socket: SocketAddr) {
        match self.close_channels.remove(&socket) {
            Some(cc) => {
                if let Err(_) = cc.send(()) {
                    error!("Error sending message to convergance agent");
                };
            }
            None => {}
        }
    }

    async fn handle_accept(
        &mut self,
        accept: Result<(TcpStream, SocketAddr), std::io::Error>,
    ) -> bool {
        match accept {
            Ok((stream, peer_addr)) => {
                info!("New connection from {}", peer_addr);
                match TCPCLSession::new(
                    stream,
                    self.settings.my_node_id.clone(),
                    self.tls_settings.clone(),
                ) {
                    Ok(sess) => {
                        self.process_socket(sess).await;
                    }
                    Err(e) => {
                        warn!("Error accepint new connection: {}", e);
                    }
                };
                return false;
            }
            Err(e) => {
                error!("Error during accepting new connection: {}", e);
                return true;
            }
        }
    }

    async fn process_socket(&mut self, mut sess: TCPCLSession) {
        let close_channel = sess.get_close_channel();
        self.close_channels
            .insert(sess.get_connection_info().peer_address, close_channel);

        let send_channel = sess.get_send_channel();

        let established_channel = sess.get_established_channel();
        let established_convergane_agent_sender =
            self.convergance_agent_sender.as_ref().unwrap().clone();
        tokio::spawn(async move {
            match established_channel.await {
                Ok(ci) => match Endpoint::new(&ci.peer_endpoint.as_ref().unwrap()) {
                    Some(node) => {
                        let bundle_sender = get_bundle_sender(send_channel);
                        if let Err(e) = established_convergane_agent_sender
                            .send(ConverganceAgentRequest::CLRegisterNode {
                                url: format!("tcpcl://{}", ci.peer_address),
                                node,
                                max_bundle_size: ci
                                    .max_bundle_size
                                    .expect("We must have a bundle size if we are connected"),
                                sender: bundle_sender,
                            })
                            .await
                        {
                            warn!(
                                "Error sending node registration to Convergance Agent: {:?}",
                                e
                            );
                            //TODO: close the session
                            return;
                        };
                    }
                    None => {
                        warn!(
                            "Peer send invalid id '{}'.",
                            ci.peer_endpoint.as_ref().unwrap()
                        );
                        //TODO: close the session
                    }
                },
                Err(_) => {}
            }
        });

        let mut transfer_receiver = sess.get_receive_channel();
        let receiver_convergane_agent_sender =
            self.convergance_agent_sender.as_ref().unwrap().clone();
        tokio::spawn(async move {
            loop {
                match transfer_receiver.recv().await {
                    Some(t) => {
                        info!("Received transfer id {}", t.id);
                        match t.data.try_into() {
                            Ok(bundle) => {
                                if let Err(e) = receiver_convergane_agent_sender
                                    .send(ConverganceAgentRequest::CLForwardBundle { bundle })
                                    .await
                                {
                                    warn!(
                                        "Error sending received bundle to Convergance Agent: {:?}",
                                        e
                                    );
                                };
                            }
                            Err(e) => {
                                warn!("Remote send invalid bundle. Dropping...: {:?}", e);
                            }
                        }
                    }
                    None => break,
                }
            }
        });

        let finished_convergane_agent_sender =
            self.convergance_agent_sender.as_ref().unwrap().clone();
        let jh = tokio::spawn(async move {
            if let Err(e) = sess.manage_connection().await {
                warn!("Connection closed with error: {:?}", e);
            }
            let ci = sess.get_connection_info();
            let node = match ci.peer_endpoint {
                Some(endpoint) => Endpoint::new(&endpoint),
                None => None,
            };
            if let Err(e) = finished_convergane_agent_sender
                .send(ConverganceAgentRequest::CLUnregisterNode {
                    url: format!("tcpcl://{}", ci.peer_address),
                    node,
                })
                .await
            {
                warn!(
                    "Error sending node unregistration to Convergance Agent: {:?}",
                    e
                );
                return;
            };
        });
        self.tcpcl_sessions.push(jh);
    }
}

fn get_bundle_sender(
    send_channel: mpsc::Sender<(Vec<u8>, oneshot::Sender<Result<(), TransferSendErrors>>)>,
) -> mpsc::Sender<crate::converganceagent::messages::AgentForwardBundle> {
    let (bundle_sender, mut bundle_receiver) = mpsc::channel::<AgentForwardBundle>(1);

    tokio::spawn(async move {
        loop {
            match bundle_receiver.recv().await {
                Some(afb) => {
                    match afb.bundle.try_into() {
                        Ok(bundle_data) => {
                            let (status_sender, status_receiver) = oneshot::channel();
                            match send_channel.send((bundle_data, status_sender)).await {
                                Ok(_) => {}
                                Err(e) => {
                                    error!("Error sending bundle to tcpcl connection. Bundle will be dropped here: {}", e);
                                }
                            }
                            match status_receiver.await {
                                Ok(status) => match status {
                                    Ok(_) => {
                                        info!("Bundle successfully sent");
                                    }
                                    Err(e) => {
                                        error!("Error sending bundle because of {:?}. Bundle will be dropped here", e);
                                    }
                                },
                                Err(e) => {
                                    error!("We could not receive a bundle status: {:?}", e);
                                }
                            }
                        }
                        Err(e) => {
                            error!(
                                "Error converting bundle to bytes. Bundle will be dropped here: {:?}",
                                e
                            );
                            //TODO: dont drop stuff :)
                        }
                    };
                }
                None => return,
            }
        }
    });

    return bundle_sender;
}
