use std::net::SocketAddr;

use log::{error, info, warn};
use tcpcl::session::TCPCLSession;
use tokio::{
    net::{TcpListener, TcpStream},
    sync::{broadcast, mpsc, oneshot},
    task::JoinHandle,
};

use crate::converganceagent::messages::ConverganceAgentRequest;

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

        let mut receiver = sess.get_receive_channel();
        let close_channel = sess.get_close_channel();
        self.close_channels.push(close_channel);

        let jh = tokio::spawn(sess.manage_connection());
        self.tcpcl_sessions.push(jh);
        tokio::spawn(async move {
            loop {
                match receiver.recv().await {
                    Some(t) => {
                        info!("Received transfer {:?}", t)
                    }
                    None => break,
                }
            }
        });
    }
}
