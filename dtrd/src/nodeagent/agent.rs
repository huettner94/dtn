// Copyright (C) 2023 Felix Huettner
//
// This file is part of DTRD.
//
// DTRD is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// DTRD is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with this program.  If not, see <https://www.gnu.org/licenses/>.

use crate::{
    converganceagent::messages::{AgentConnectNode, AgentDisconnectNode},
    routingagent::messages::{AddRoute, RemoveRoute, RouteType},
};
use actix::prelude::*;
use log::{info, warn};
use std::time::Duration;

use super::messages::{
    AddNode, ListNodes, Node, NodeConnectionStatus, NotifyNodeConnected, NotifyNodeDisconnected,
    RemoveNode, TryConnect,
};

#[derive(Default)]
pub struct Daemon {
    nodes: Vec<Node>,
}

impl Actor for Daemon {
    type Context = Context<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        ctx.run_interval(Duration::from_secs(60), |_, ctx| {
            ctx.notify(TryConnect {});
        });
    }
}

impl actix::Supervised for Daemon {}

impl SystemService for Daemon {}

impl Handler<ListNodes> for Daemon {
    type Result = Vec<Node>;

    fn handle(&mut self, _msg: ListNodes, _ctx: &mut Context<Self>) -> Self::Result {
        self.nodes.clone()
    }
}

impl Handler<AddNode> for Daemon {
    type Result = ();

    fn handle(&mut self, msg: AddNode, _ctx: &mut Context<Self>) -> Self::Result {
        let AddNode { url } = msg;
        let mut node = Node {
            url: url.clone(),
            connection_status: NodeConnectionStatus::Disconnected,
            remote_endpoint: None,
            temporary: false,
        };
        if !self.nodes.contains(&node) {
            node.connection_status = NodeConnectionStatus::Connecting;
            self.nodes.push(node);
            crate::converganceagent::agent::Daemon::from_registry()
                .do_send(AgentConnectNode { url });
        }
    }
}

impl Handler<RemoveNode> for Daemon {
    type Result = ();

    fn handle(&mut self, msg: RemoveNode, _ctx: &mut Context<Self>) -> Self::Result {
        let RemoveNode { url } = msg;
        let node = Node {
            url: url.clone(),
            connection_status: NodeConnectionStatus::Disconnected,
            remote_endpoint: None,
            temporary: false,
        };
        if let Some(pos) = self.nodes.iter().position(|x| x == &node) {
            let node = &mut self.nodes[pos];
            node.temporary = true;
            node.connection_status = NodeConnectionStatus::Disconnecting;

            crate::converganceagent::agent::Daemon::from_registry()
                .do_send(AgentDisconnectNode { url });
        }
    }
}

impl Handler<NotifyNodeConnected> for Daemon {
    type Result = ();

    fn handle(&mut self, msg: NotifyNodeConnected, _ctx: &mut Context<Self>) -> Self::Result {
        let NotifyNodeConnected {
            url,
            endpoint,
            max_bundle_size,
        } = msg;
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
        crate::routingagent::agent::Daemon::from_registry().do_send(AddRoute {
            target: endpoint.clone(),
            route_type: RouteType::Connected,
            next_hop: endpoint,
            max_bundle_size: Some(max_bundle_size),
        });
    }
}

impl Handler<NotifyNodeDisconnected> for Daemon {
    type Result = ();

    fn handle(&mut self, msg: NotifyNodeDisconnected, _ctx: &mut Context<Self>) -> Self::Result {
        let NotifyNodeDisconnected { url } = msg;
        match self.nodes.iter().position(|n| n.url == url) {
            Some(pos) => {
                let node = &mut self.nodes[pos];

                if node.remote_endpoint.is_some() {
                    crate::routingagent::agent::Daemon::from_registry().do_send(RemoveRoute {
                        target: node.remote_endpoint.clone().unwrap(),
                        route_type: RouteType::Connected,
                        next_hop: node.remote_endpoint.clone().unwrap(),
                    });
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
                    "We received a node disconnect info, but dont know about the node: {url}"
                );
            }
        }
    }
}

impl Handler<TryConnect> for Daemon {
    type Result = ();

    fn handle(&mut self, _msg: TryConnect, _ctx: &mut Context<Self>) -> Self::Result {
        for node in &mut self.nodes {
            if node.connection_status == NodeConnectionStatus::Disconnected && !node.temporary {
                info!("Trying to reconnect to {}", node.url);
                node.connection_status = NodeConnectionStatus::Connecting;
                crate::converganceagent::agent::Daemon::from_registry().do_send(AgentConnectNode {
                    url: node.url.clone(),
                });
            }
        }
    }
}
