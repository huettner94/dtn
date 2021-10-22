use std::collections::HashMap;

use async_trait::async_trait;
use bp7::endpoint::Endpoint;
use log::{info, warn};
use tokio::sync::{mpsc, oneshot};
use tokio_util::sync::CancellationToken;

use super::messages::{AgentForwardBundle, ConverganceAgentRequest};

pub struct Daemon {
    connected_nodes: HashMap<Endpoint, (mpsc::Sender<AgentForwardBundle>, CancellationToken)>,
    channel_receiver: Option<mpsc::Receiver<ConverganceAgentRequest>>,
}

#[async_trait]
impl crate::common::agent::Daemon for Daemon {
    type MessageType = ConverganceAgentRequest;

    fn new() -> Self {
        Daemon {
            connected_nodes: HashMap::new(),
            channel_receiver: None,
        }
    }

    fn get_agent_name(&self) -> &'static str {
        "ConverganceAgent"
    }

    fn get_channel_receiver(&mut self) -> Option<mpsc::Receiver<Self::MessageType>> {
        self.channel_receiver.take()
    }

    async fn on_shutdown(&mut self) {
        info!("Closing all Convergance agent channels");
        for (node_endpoint, node_sender) in self.connected_nodes.drain() {
            drop(node_sender);
            info!("Closed node channel for {:?}", node_endpoint);
        }
    }

    async fn handle_message(&mut self, msg: ConverganceAgentRequest) {
        match msg {
            ConverganceAgentRequest::AgentGetNode {
                destination,
                responder,
            } => {
                self.message_agent_get_node(destination, responder).await;
            }
        }
    }
}

impl Daemon {
    pub fn init_channels(&mut self) -> mpsc::Sender<ConverganceAgentRequest> {
        let (channel_sender, channel_receiver) = mpsc::channel::<ConverganceAgentRequest>(1);
        self.channel_receiver = Some(channel_receiver);
        return channel_sender;
    }

    async fn message_agent_get_node(
        &mut self,
        destination: Endpoint,
        responder: oneshot::Sender<Option<mpsc::Sender<AgentForwardBundle>>>,
    ) {
        let dst = destination.get_node_endpoint();
        let resp = match self.connected_nodes.get(&dst) {
            Some((sender, canceltoken)) => {
                if canceltoken.is_cancelled() {
                    info!("Node for endpoint {} already disconnected", dst);
                    self.connected_nodes.remove(&dst);
                    None
                } else {
                    Some(sender.clone())
                }
            }
            None => None,
        };
        if let Err(e) = responder.send(resp) {
            warn!("Error sending Convergance get back to requestor: {:?}", e);
        }
    }
}
