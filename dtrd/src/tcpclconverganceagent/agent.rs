use std::net::SocketAddr;

use bp7::endpoint::Endpoint;
use log::{error, info, warn};
use tcpcl::session::TCPCLSession;
use tokio::{
    net::{TcpListener, TcpStream},
    sync::{broadcast, mpsc, oneshot},
    task::JoinHandle,
};

use crate::converganceagent::messages::{AgentForwardBundle, ConverganceAgentRequest};

pub struct Daemon {
    convergance_agent_sender: Option<mpsc::Sender<ConverganceAgentRequest>>,
    tcpcl_sessions: Vec<JoinHandle<()>>,
    close_channels: Vec<oneshot::Sender<()>>,
}

impl Daemon {
    pub fn new() -> Self {
        Daemon {
            convergance_agent_sender: None,
            tcpcl_sessions: Vec::new(),
            close_channels: Vec::new(),
        }
    }

    pub fn init_channels(
        &mut self,
        convergance_agent_sender: mpsc::Sender<ConverganceAgentRequest>,
    ) {
        self.convergance_agent_sender = Some(convergance_agent_sender);
    }

    pub async fn run(
        &mut self,
        mut shutdown: broadcast::Receiver<()>,
        _sender: mpsc::Sender<()>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let socket: SocketAddr = "[::1]:4556".parse().unwrap();

        info!("Server listening on {}", socket);

        let listener = TcpListener::bind(&socket).await?;
        info!("Socket open, waiting for connection");
        loop {
            tokio::select! {
                res = listener.accept() => {
                    if self.handle_accept(res).await {
                        break;
                    }
                }
                _ = shutdown.recv() => {
                    info!("TCPCL agent received shutdown");
                    break;
                }
            }
        }

        info!("Closing all tcpcl sessions");
        for close_channel in self.close_channels.drain(..) {
            match close_channel.send(()) {
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

        info!("TCPCL Server has shutdown. See you");
        // _sender is explicitly dropped here
        Ok(())
    }

    async fn handle_accept(
        &mut self,
        accept: Result<(TcpStream, SocketAddr), std::io::Error>,
    ) -> bool {
        match accept {
            Ok((stream, peer_addr)) => {
                info!("New connection from {}", peer_addr);
                self.process_socket(stream).await;
                return false;
            }
            Err(e) => {
                error!("Error during accepting new connection: {}", e);
                return true;
            }
        }
    }

    async fn process_socket(&mut self, stream: TcpStream) {
        let mut sess = TCPCLSession::new(stream);

        let close_channel = sess.get_close_channel();
        self.close_channels.push(close_channel);

        let send_channel = sess.get_send_channel();

        let established_channel = sess.get_established_channel();
        let established_convergane_agent_sender =
            self.convergance_agent_sender.as_ref().unwrap().clone();
        tokio::spawn(async move {
            match established_channel.await {
                Ok(ci) => match Endpoint::new(&ci.peer_endpoint) {
                    Some(node) => {
                        let bundle_sender = get_bundle_sender(send_channel);
                        if let Err(e) = established_convergane_agent_sender
                            .send(ConverganceAgentRequest::CLRegisterNode {
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
                        warn!("Peer send invalid id '{}'.", ci.peer_endpoint);
                        //TODO: close the session
                    }
                },
                Err(_) => {}
            }
        });

        let mut transfer_receiver = sess.get_receive_channel();
        tokio::spawn(async move {
            loop {
                match transfer_receiver.recv().await {
                    Some(t) => {
                        info!("Received transfer {:?}", t)
                    }
                    None => break,
                }
            }
        });

        let finished_convergane_agent_sender =
            self.convergance_agent_sender.as_ref().unwrap().clone();
        let jh = tokio::spawn(async move {
            sess.manage_connection().await;
            match sess.get_connection_info() {
                Some(ci) => match Endpoint::new(&ci.peer_endpoint) {
                    Some(node) => {
                        if let Err(e) = finished_convergane_agent_sender
                            .send(ConverganceAgentRequest::CLUnregisterNode { node })
                            .await
                        {
                            warn!(
                                "Error sending node unregistration to Convergance Agent: {:?}",
                                e
                            );
                            //TODO: close the session
                            return;
                        };
                    }
                    None => {
                        warn!("Peer send invalid id '{}'.", ci.peer_endpoint);
                        //TODO: close the session
                    }
                },
                None => {}
            }
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
