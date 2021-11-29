use std::time::Duration;

use async_trait::async_trait;
use bp7::endpoint::Endpoint;
use log::{error, info, warn};
use tokio::{
    sync::{mpsc, oneshot},
    time::interval,
};

use crate::{
    common::settings::Settings,
    common::shutdown::Shutdown,
    converganceagent::messages::ConverganceAgentRequest,
    routingagent::messages::{RouteType, RoutingAgentRequest},
};

use super::messages::{Node, NodeAgentRequest, NodeConnectionStatus};

pub struct Daemon {
    nodes: Vec<Node>,
    channel_receiver: Option<mpsc::Receiver<NodeAgentRequest>>,
    convergance_agent_sender: Option<mpsc::Sender<ConverganceAgentRequest>>,
    routing_agent_sender: Option<mpsc::Sender<RoutingAgentRequest>>,
}

#[async_trait]
impl crate::common::agent::Daemon for Daemon {
    type MessageType = NodeAgentRequest;

    async fn new(_: &Settings) -> Self {
        Daemon {
            nodes: Vec::new(),
            channel_receiver: None,
            convergance_agent_sender: None,
            routing_agent_sender: None,
        }
    }

    fn get_agent_name(&self) -> &'static str {
        "Node Agent"
    }

    fn get_channel_receiver(&mut self) -> Option<mpsc::Receiver<Self::MessageType>> {
        self.channel_receiver.take()
    }

    async fn main_loop(
        &mut self,
        shutdown: &mut Shutdown,
        receiver: &mut mpsc::Receiver<Self::MessageType>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut reconnect_interval = interval(Duration::from_secs(60));
        while !shutdown.is_shutdown() {
            tokio::select! {
                res = receiver.recv() => {
                    if let Some(msg) = res {
                        self.handle_message(msg).await;
                    } else {
                        info!("{} can no longer receive messages. Exiting", self.get_agent_name());
                        return Ok(())
                    }
                }
                _ = shutdown.recv() => {
                    info!("{} received shutdown", self.get_agent_name());
                    receiver.close();
                    info!("{} will not allow more requests to be sent", self.get_agent_name());
                }
                _ = reconnect_interval.tick() => {
                    self.handle_reconnect().await;
                }
            }
        }
        Ok(())
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
        routing_agent_sender: mpsc::Sender<RoutingAgentRequest>,
    ) -> mpsc::Sender<NodeAgentRequest> {
        self.convergance_agent_sender = Some(convergance_agent_sender);
        self.routing_agent_sender = Some(routing_agent_sender);
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
            url: url.clone(),
            connection_status: NodeConnectionStatus::Disconnected,
            remote_endpoint: None,
            temporary: false,
        };
        match self.nodes.iter().position(|x| x == &node) {
            Some(pos) => {
                let node = &mut self.nodes[pos];
                node.temporary = true;
                node.connection_status = NodeConnectionStatus::Disconnecting;

                if let Err(e) = self
                    .convergance_agent_sender
                    .as_ref()
                    .unwrap()
                    .send(ConverganceAgentRequest::AgentDisconnectNode {
                        connection_string: url,
                    })
                    .await
                {
                    error!("Error sending request to convergance agent: {:?}", e)
                }
            }
            None => {}
        }
    }

    async fn message_notify_node_connected(&mut self, url: String, endpoint: Endpoint) {
        match self.nodes.iter().position(|n| n.url == url) {
            Some(pos) => {
                let node = &mut self.nodes[pos];
                node.connection_status = NodeConnectionStatus::Connected;
                node.remote_endpoint = Some(endpoint.clone());
            }
            None => {
                self.nodes.push(Node {
                    url,
                    connection_status: NodeConnectionStatus::Connected,
                    remote_endpoint: Some(endpoint.clone()),
                    temporary: true,
                });
            }
        }
        if let Err(e) = self
            .routing_agent_sender
            .as_ref()
            .unwrap()
            .send(RoutingAgentRequest::AddRoute {
                target: endpoint.clone(),
                route_type: RouteType::Connected,
                next_hop: endpoint,
            })
            .await
        {
            error!(
                "Error sending node add notification to routing agent: {:?}",
                e
            );
        }
    }

    async fn message_notify_node_disconnected(&mut self, url: String) {
        match self.nodes.iter().position(|n| n.url == url) {
            Some(pos) => {
                let node = &mut self.nodes[pos];

                if node.remote_endpoint.is_some() {
                    if let Err(e) = self
                        .routing_agent_sender
                        .as_ref()
                        .unwrap()
                        .send(RoutingAgentRequest::RemoveRoute {
                            target: node.remote_endpoint.clone().unwrap(),
                            route_type: RouteType::Connected,
                            next_hop: node.remote_endpoint.clone().unwrap(),
                        })
                        .await
                    {
                        error!(
                            "Error sending node remove notification to routing agent: {:?}",
                            e
                        );
                    }
                }

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

    async fn handle_reconnect(&mut self) {
        for node in &mut self.nodes {
            if node.connection_status == NodeConnectionStatus::Disconnected && !node.temporary {
                info!("Trying to reconnect to {}", node.url);
                node.connection_status = NodeConnectionStatus::Connecting;
                if let Err(e) = self
                    .convergance_agent_sender
                    .as_ref()
                    .unwrap()
                    .send(ConverganceAgentRequest::AgentConnectNode {
                        connection_string: node.url.clone(),
                    })
                    .await
                {
                    error!("Error sending request to convergance agent: {:?}", e)
                }
            }
        }
    }
}
