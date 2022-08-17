use std::collections::HashMap;

use bp7::endpoint::Endpoint;
use log::{error, info};
use tokio::sync::mpsc;

use crate::{
    bundleprotocolagent::messages::ReceiveBundle,
    nodeagent::messages::{NotifyNodeConnected, NotifyNodeDisconnected},
};

use super::messages::{
    AgentConnectNode, AgentDisconnectNode, AgentForwardBundle, AgentGetNode, CLForwardBundle,
    CLRegisterNode, CLUnregisterNode,
};
use actix::prelude::*;

#[derive(Default)]
pub struct Daemon {
    connected_nodes: HashMap<Endpoint, mpsc::Sender<AgentForwardBundle>>,
}

impl Actor for Daemon {
    type Context = Context<Self>;

    fn stopped(&mut self, ctx: &mut Context<Self>) {
        info!("Closing all Convergance agent channels");
        for (node_endpoint, node_sender) in self.connected_nodes.drain() {
            drop(node_sender);
            info!("Closed node channel for {:?}", node_endpoint);
        }
    }
}

impl actix::Supervised for Daemon {}

impl SystemService for Daemon {}

impl Handler<AgentGetNode> for Daemon {
    type Result = Option<mpsc::Sender<AgentForwardBundle>>;

    fn handle(&mut self, msg: AgentGetNode, ctx: &mut Context<Self>) -> Self::Result {
        let AgentGetNode { destination } = msg;
        let dst = destination.get_node_endpoint();
        let resp = match self.connected_nodes.get(&dst) {
            Some(sender) => Some(sender.clone()),
            None => None,
        };
        resp
    }
}

impl Handler<AgentConnectNode> for Daemon {
    type Result = ();

    fn handle(&mut self, msg: AgentConnectNode, ctx: &mut Context<Self>) -> Self::Result {
        let AgentConnectNode { connection_string } = msg;
        match connection_string.split_once(':') {
            Some((proto, hostport)) => match proto {
                "tcpcl" => match hostport[2..].parse() {
                    Ok(socket) => {
                        panic!("Should now send a reqeust to the tcpcl agent");
                        /*if let Err(e) = self
                            .tcpcl_agent_sender
                            .as_ref()
                            .unwrap()
                            .send(TCPCLAgentRequest::ConnectRemote { socket })
                        {
                            error!("Error sending request to tcpcl agent {:?}", e);
                        }*/
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
}

impl Handler<AgentDisconnectNode> for Daemon {
    type Result = ();

    fn handle(&mut self, msg: AgentDisconnectNode, ctx: &mut Context<Self>) -> Self::Result {
        let AgentDisconnectNode { connection_string } = msg;
        match connection_string.split_once(':') {
            Some((proto, hostport)) => match proto {
                "tcpcl" => match hostport[2..].parse() {
                    Ok(socket) => {
                        panic!("Should now send a reqeust to the tcpcl agent");
                        /*if let Err(e) = self
                            .tcpcl_agent_sender
                            .as_ref()
                            .unwrap()
                            .send(TCPCLAgentRequest::DisonnectRemote { socket })
                        {
                            error!("Error sending request to tcpcl agent {:?}", e);
                        }*/
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
}

impl Handler<CLRegisterNode> for Daemon {
    type Result = ();

    fn handle(&mut self, msg: CLRegisterNode, ctx: &mut Context<Self>) -> Self::Result {
        let CLRegisterNode {
            url,
            node,
            max_bundle_size,
            sender,
        } = msg;
        info!("Received a registration request for node {}", node);
        self.connected_nodes.insert(node.clone(), sender);
        crate::nodeagent::agent::Daemon::from_registry().do_send(NotifyNodeConnected {
            url,
            endpoint: node.clone(),
            max_bundle_size,
        });
    }
}

impl Handler<CLUnregisterNode> for Daemon {
    type Result = ();

    fn handle(&mut self, msg: CLUnregisterNode, ctx: &mut Context<Self>) -> Self::Result {
        let CLUnregisterNode { url, node } = msg;
        info!(
            "Received an unregistration request for node {:?} at url {}",
            node, url
        );
        if node.is_some() {
            self.connected_nodes.remove(&node.unwrap());
        }
        crate::nodeagent::agent::Daemon::from_registry().do_send(NotifyNodeDisconnected { url });
    }
}

impl Handler<CLForwardBundle> for Daemon {
    type Result = ();

    fn handle(&mut self, msg: CLForwardBundle, ctx: &mut Context<Self>) -> Self::Result {
        let CLForwardBundle { bundle, responder } = msg;
        crate::bundleprotocolagent::agent::Daemon::from_registry()
            .do_send(ReceiveBundle { bundle, responder });
    }
}
