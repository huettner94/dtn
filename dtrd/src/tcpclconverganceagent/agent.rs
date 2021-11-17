use std::{collections::HashMap, net::SocketAddr};

use async_trait::async_trait;

use bp7::endpoint::Endpoint;
use log::{error, info, warn};
use tcpcl::session::TCPCLSession;
use tokio::{
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
    channel_receiver: Option<mpsc::Receiver<TCPCLAgentRequest>>,
    convergance_agent_sender: Option<mpsc::Sender<ConverganceAgentRequest>>,
    tcpcl_sessions: Vec<JoinHandle<()>>,
    close_channels: HashMap<SocketAddr, oneshot::Sender<()>>,
}

#[async_trait]
impl crate::common::agent::Daemon for Daemon {
    type MessageType = TCPCLAgentRequest;

    fn new(settings: &Settings) -> Self {
        Daemon {
            settings: settings.clone(),
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
        match TCPCLSession::connect(socket, self.settings.my_node_id.clone(), None).await {
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
                match TCPCLSession::new(stream, self.settings.my_node_id.clone(), None) {
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
            sess.manage_connection().await;
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
    send_channel: mpsc::Sender<Vec<u8>>,
) -> mpsc::Sender<crate::converganceagent::messages::AgentForwardBundle> {
    let (bundle_sender, mut bundle_receiver) = mpsc::channel::<AgentForwardBundle>(1);

    tokio::spawn(async move {
        loop {
            match bundle_receiver.recv().await {
                Some(afb) => {
                    match afb.bundle.try_into() {
                        Ok(bundle_data) => match send_channel.send(bundle_data).await {
                            Ok(_) => {}
                            Err(e) => {
                                error!("Error sending bundle to tcpcl connection. Bundle will be dropped here: {}", e);
                            }
                        },
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
