use std::{collections::HashMap, net::SocketAddr};

use bp7::endpoint::Endpoint;
use log::{error, info};

use crate::{
    converganceagent::messages::{EventPeerConnected, EventPeerDisconnected},
    nodeagent::messages::{NotifyNodeConnected, NotifyNodeDisconnected},
    tcpclconverganceagent::messages::{ConnectRemote, DisconnectRemote},
};

use super::messages::{
    AgentConnectNode, AgentDisconnectNode, AgentForwardBundle, CLRegisterNode, CLUnregisterNode,
};
use actix::prelude::*;

#[derive(Default)]
pub struct Daemon {
    connected_nodes: HashMap<Endpoint, Recipient<AgentForwardBundle>>,
}

impl Actor for Daemon {
    type Context = Context<Self>;

    fn stopped(&mut self, _ctx: &mut Context<Self>) {
        info!("Closing all Convergance agent channels");
        for (node_endpoint, node_sender) in self.connected_nodes.drain() {
            drop(node_sender);
            info!("Closed node channel for {:?}", node_endpoint);
        }
    }
}

impl actix::Supervised for Daemon {}

impl SystemService for Daemon {}

impl Handler<AgentConnectNode> for Daemon {
    type Result = ();

    fn handle(&mut self, msg: AgentConnectNode, _ctx: &mut Context<Self>) -> Self::Result {
        let AgentConnectNode { connection_string } = msg;
        match connection_string.split_once(':') {
            Some((proto, hostport)) => match proto {
                "tcpcl" => match hostport[2..].parse::<SocketAddr>() {
                    Ok(address) => {
                        crate::tcpclconverganceagent::agent::TCPCLServer::from_registry()
                            .do_send(ConnectRemote { address });
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

    fn handle(&mut self, msg: AgentDisconnectNode, _ctx: &mut Context<Self>) -> Self::Result {
        let AgentDisconnectNode { connection_string } = msg;
        match connection_string.split_once(':') {
            Some((proto, hostport)) => match proto {
                "tcpcl" => match hostport[2..].parse::<SocketAddr>() {
                    Ok(address) => {
                        crate::tcpclconverganceagent::agent::TCPCLServer::from_registry()
                            .do_send(DisconnectRemote { address });
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

    fn handle(&mut self, msg: CLRegisterNode, _ctx: &mut Context<Self>) -> Self::Result {
        let CLRegisterNode {
            url,
            node,
            max_bundle_size,
            sender,
        } = msg;
        info!("Received a registration request for node {}", node);
        self.connected_nodes.insert(node.clone(), sender.clone());
        crate::nodeagent::agent::Daemon::from_registry().do_send(NotifyNodeConnected {
            url,
            endpoint: node.clone(),
            max_bundle_size,
        });
        crate::bundleprotocolagent::agent::Daemon::from_registry().do_send(EventPeerConnected {
            destination: node,
            sender,
        });
    }
}

impl Handler<CLUnregisterNode> for Daemon {
    type Result = ();

    fn handle(&mut self, msg: CLUnregisterNode, _ctx: &mut Context<Self>) -> Self::Result {
        let CLUnregisterNode { url, node } = msg;
        info!(
            "Received an unregistration request for node {:?} at url {}",
            node, url
        );
        if let Some(dstnode) = &node {
            self.connected_nodes.remove(&node.clone().unwrap());
            crate::bundleprotocolagent::agent::Daemon::from_registry().do_send(
                EventPeerDisconnected {
                    destination: dstnode.clone(),
                },
            );
        }
        crate::nodeagent::agent::Daemon::from_registry().do_send(NotifyNodeDisconnected { url });
    }
}
