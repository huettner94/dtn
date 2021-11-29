use std::collections::HashMap;

use async_trait::async_trait;
use bp7::{bundle::Bundle, endpoint::Endpoint};
use log::{error, info, warn};
use tokio::sync::{mpsc, oneshot};

use crate::{
    bundleprotocolagent::messages::BPARequest, common::settings::Settings,
    nodeagent::messages::NodeAgentRequest, tcpclconverganceagent::messages::TCPCLAgentRequest,
};

use super::messages::{AgentForwardBundle, ConverganceAgentRequest};

pub struct Daemon {
    connected_nodes: HashMap<Endpoint, mpsc::Sender<AgentForwardBundle>>,
    channel_receiver: Option<mpsc::Receiver<ConverganceAgentRequest>>,
    bpa_sender: Option<mpsc::Sender<BPARequest>>,
    node_agent_sender: Option<mpsc::Sender<NodeAgentRequest>>,
    tcpcl_agent_sender: Option<mpsc::Sender<TCPCLAgentRequest>>,
}

#[async_trait]
impl crate::common::agent::Daemon for Daemon {
    type MessageType = ConverganceAgentRequest;

    async fn new(_: &Settings) -> Self {
        Daemon {
            connected_nodes: HashMap::new(),
            channel_receiver: None,
            bpa_sender: None,
            node_agent_sender: None,
            tcpcl_agent_sender: None,
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
            ConverganceAgentRequest::AgentConnectNode { connection_string } => {
                self.message_agent_connect_node(connection_string).await;
            }
            ConverganceAgentRequest::AgentDisconnectNode { connection_string } => {
                self.message_agent_disconnect_node(connection_string).await;
            }
            ConverganceAgentRequest::CLRegisterNode { url, node, sender } => {
                self.message_cl_register_node(url, node, sender).await;
            }
            ConverganceAgentRequest::CLUnregisterNode { url, node } => {
                self.message_cl_unregister_node(url, node).await;
            }
            ConverganceAgentRequest::CLForwardBundle { bundle } => {
                self.message_cl_forward_bundle(bundle).await;
            }
        }
    }
}

impl Daemon {
    pub fn init_channels(
        &mut self,
        bpa_sender: mpsc::Sender<BPARequest>,
    ) -> mpsc::Sender<ConverganceAgentRequest> {
        self.bpa_sender = Some(bpa_sender);
        let (channel_sender, channel_receiver) = mpsc::channel::<ConverganceAgentRequest>(1);
        self.channel_receiver = Some(channel_receiver);
        return channel_sender;
    }

    pub fn set_senders(
        &mut self,
        node_agent_sender: mpsc::Sender<NodeAgentRequest>,
        tcpcl_agent_sender: mpsc::Sender<TCPCLAgentRequest>,
    ) {
        self.node_agent_sender = Some(node_agent_sender);
        self.tcpcl_agent_sender = Some(tcpcl_agent_sender);
    }

    async fn message_agent_get_node(
        &mut self,
        destination: Endpoint,
        responder: oneshot::Sender<Option<mpsc::Sender<AgentForwardBundle>>>,
    ) {
        let dst = destination.get_node_endpoint();
        let resp = match self.connected_nodes.get(&dst) {
            Some(sender) => Some(sender.clone()),
            None => None,
        };
        if let Err(e) = responder.send(resp) {
            warn!("Error sending Convergance get back to requestor: {:?}", e);
        }
    }

    async fn message_agent_connect_node(&mut self, connection_string: String) {
        match connection_string.split_once(':') {
            Some((proto, hostport)) => match proto {
                "tcpcl" => match hostport[2..].parse() {
                    Ok(socket) => {
                        if let Err(e) = self
                            .tcpcl_agent_sender
                            .as_ref()
                            .unwrap()
                            .send(TCPCLAgentRequest::ConnectRemote { socket })
                            .await
                        {
                            error!("Error sending request to tcpcl agent {:?}", e);
                        }
                    }
                    Err(e) => {
                        error!(
                            "Invalid address '{}' specified to connect to new nodes: {}",
                            hostport[2..].to_string(),
                            e
                        );
                        //TODO make a response to the requestor
                    }
                },
                _ => {
                    error!("Invalid protocol: {}", proto);
                    //TODO make a response to the requestor
                }
            },
            None => {
                error!("Invalid connection string format: {}", connection_string);
                //TODO make a response to the requestor
            }
        }
    }

    async fn message_agent_disconnect_node(&mut self, connection_string: String) {
        match connection_string.split_once(':') {
            Some((proto, hostport)) => match proto {
                "tcpcl" => match hostport[2..].parse() {
                    Ok(socket) => {
                        if let Err(e) = self
                            .tcpcl_agent_sender
                            .as_ref()
                            .unwrap()
                            .send(TCPCLAgentRequest::DisonnectRemote { socket })
                            .await
                        {
                            error!("Error sending request to tcpcl agent {:?}", e);
                        }
                    }
                    Err(e) => {
                        error!(
                            "Invalid address '{}' specified to connect to new nodes: {}",
                            hostport[2..].to_string(),
                            e
                        );
                        //TODO make a response to the requestor
                    }
                },
                _ => {
                    error!("Invalid protocol: {}", proto);
                    //TODO make a response to the requestor
                }
            },
            None => {
                error!("Invalid connection string format: {}", connection_string);
                //TODO make a response to the requestor
            }
        }
    }

    async fn message_cl_register_node(
        &mut self,
        url: String,
        node: Endpoint,
        sender: mpsc::Sender<AgentForwardBundle>,
    ) {
        info!("Received a registration request for node {}", node);
        self.connected_nodes.insert(node.clone(), sender);
        if let Err(e) = self
            .node_agent_sender
            .as_ref()
            .unwrap()
            .send(NodeAgentRequest::NotifyNodeConnected {
                url,
                endpoint: node.clone(),
            })
            .await
        {
            warn!(
                "Error sending node connected notification to nodeagent: {:?}",
                e
            );
        }
    }

    async fn message_cl_unregister_node(&mut self, url: String, node: Option<Endpoint>) {
        info!(
            "Received an unregistration request for node {:?} at url {}",
            node, url
        );
        if node.is_some() {
            self.connected_nodes.remove(&node.unwrap());
        }
        if let Err(e) = self
            .node_agent_sender
            .as_ref()
            .unwrap()
            .send(NodeAgentRequest::NotifyNodeDisconnected { url })
            .await
        {
            warn!(
                "Error sending node connected notification to nodeagent: {:?}",
                e
            );
        }
    }

    async fn message_cl_forward_bundle(&mut self, bundle: Bundle) {
        match self
            .bpa_sender
            .as_ref()
            .unwrap()
            .send(BPARequest::ReceiveBundle { bundle })
            .await
        {
            Ok(_) => {}
            Err(e) => {
                warn!(
                    "Error sending Bundle to bpa. Bundle will be dropped: {:?}",
                    e
                );
            }
        }
    }
}
