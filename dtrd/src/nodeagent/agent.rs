use async_trait::async_trait;
use bp7::endpoint::Endpoint;
use log::{error, warn};
use tokio::sync::{mpsc, oneshot};

use crate::{common::settings::Settings, converganceagent::messages::ConverganceAgentRequest};

use super::messages::{Node, NodeAgentRequest, NodeConnectionStatus};

pub struct Daemon {
    nodes: Vec<Node>,
    channel_receiver: Option<mpsc::Receiver<NodeAgentRequest>>,
    convergance_agent_sender: Option<mpsc::Sender<ConverganceAgentRequest>>,
}

#[async_trait]
impl crate::common::agent::Daemon for Daemon {
    type MessageType = NodeAgentRequest;

    fn new(_: &Settings) -> Self {
        Daemon {
            nodes: Vec::new(),
            channel_receiver: None,
            convergance_agent_sender: None,
        }
    }

    fn get_agent_name(&self) -> &'static str {
        "Node Agent"
    }

    fn get_channel_receiver(&mut self) -> Option<mpsc::Receiver<Self::MessageType>> {
        self.channel_receiver.take()
    }

    async fn handle_message(&mut self, msg: NodeAgentRequest) {
        match msg {
            NodeAgentRequest::ListNodes { responder } => self.message_list_nodes(responder).await,
            NodeAgentRequest::AddNode { url } => self.message_add_node(url).await,
            NodeAgentRequest::RemoveNode { url } => self.message_remove_node(url).await,
            NodeAgentRequest::NotifyNodeConnected { url, endpoint } => {
                self.message_notify_node_connected(url, endpoint).await
            }
            NodeAgentRequest::NotifyNodeDisconnected { url } => {
                self.message_notify_node_disconnected(url).await
            }
        }
    }
}

impl Daemon {
    pub fn init_channels(
        &mut self,
        convergance_agent_sender: mpsc::Sender<ConverganceAgentRequest>,
    ) -> mpsc::Sender<NodeAgentRequest> {
        self.convergance_agent_sender = Some(convergance_agent_sender);
        let (channel_sender, channel_receiver) = mpsc::channel::<NodeAgentRequest>(1);
        self.channel_receiver = Some(channel_receiver);
        return channel_sender;
    }

    async fn message_list_nodes(&self, responder: oneshot::Sender<Vec<Node>>) {
        match responder.send(self.nodes.clone()) {
            Ok(_) => {}
            Err(e) => warn!("Error sending response for listing nodes {:?}", e),
        }
    }

    async fn message_add_node(&mut self, url: String) {
        let mut node = Node {
            url: url.clone(),
            connection_status: NodeConnectionStatus::Disconnected,
            remote_endpoint: None,
            temporary: false,
        };
        if !self.nodes.contains(&node) {
            node.connection_status = NodeConnectionStatus::Connecting;
            self.nodes.push(node);
            if let Err(e) = self
                .convergance_agent_sender
                .as_ref()
                .unwrap()
                .send(ConverganceAgentRequest::AgentConnectNode {
                    connection_string: url,
                })
                .await
            {
                error!("Error sending request to convergance agent: {:?}", e)
            }
        }
    }

    async fn message_remove_node(&mut self, url: String) {
        let node = Node {
            url,
            connection_status: NodeConnectionStatus::Disconnected,
            remote_endpoint: None,
            temporary: false,
        };
        self.nodes.retain(|x| x != &node);
    }

    async fn message_notify_node_connected(&mut self, url: String, endpoint: Endpoint) {
        match self.nodes.iter().position(|n| n.url == url) {
            Some(pos) => {
                let node = &mut self.nodes[pos];
                node.connection_status = NodeConnectionStatus::Connected;
                node.remote_endpoint = Some(endpoint);
            }
            None => {
                self.nodes.push(Node {
                    url,
                    connection_status: NodeConnectionStatus::Connected,
                    remote_endpoint: Some(endpoint),
                    temporary: true,
                });
            }
        }
    }

    async fn message_notify_node_disconnected(&mut self, url: String) {
        match self.nodes.iter().position(|n| n.url == url) {
            Some(pos) => {
                let node = &mut self.nodes[pos];
                if node.temporary {
                    self.nodes.remove(pos);
                } else {
                    node.connection_status = NodeConnectionStatus::Disconnected;
                    node.remote_endpoint = None;
                }
            }
            None => {
                warn!(
                    "We received a node disconnect info, but dont know about the node: {}",
                    url
                );
            }
        }
    }
}
