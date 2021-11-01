use std::collections::{HashMap, HashSet};

use async_trait::async_trait;
use bp7::endpoint::Endpoint;
use log::error;
use tokio::sync::{mpsc, oneshot};

use crate::common::settings::Settings;

use super::messages::{RouteType, RoutingAgentRequest};

#[derive(Debug, PartialEq, Eq, Hash)]
struct RouteEntry {
    route_type: RouteType,
    next_hop: Endpoint,
}

pub struct Daemon {
    routes: HashMap<Endpoint, HashSet<RouteEntry>>,
    channel_receiver: Option<mpsc::Receiver<RoutingAgentRequest>>,
}

#[async_trait]
impl crate::common::agent::Daemon for Daemon {
    type MessageType = RoutingAgentRequest;

    fn new(_: &Settings) -> Self {
        Daemon {
            routes: HashMap::new(),
            channel_receiver: None,
        }
    }

    fn get_agent_name(&self) -> &'static str {
        "Routing Agent"
    }

    fn get_channel_receiver(&mut self) -> Option<mpsc::Receiver<Self::MessageType>> {
        self.channel_receiver.take()
    }

    async fn handle_message(&mut self, msg: RoutingAgentRequest) {
        match msg {
            RoutingAgentRequest::AddRoute {
                target,
                route_type,
                next_hop,
            } => self.message_add_route(target, route_type, next_hop),
            RoutingAgentRequest::RemoveRoute {
                target,
                route_type,
                next_hop,
            } => self.message_remove_route(target, route_type, next_hop),
            RoutingAgentRequest::GetNextHop { target, responder } => {
                self.message_get_next_hop(target, responder)
            }
        }
    }
}

impl Daemon {
    pub fn init_channels(&mut self) -> mpsc::Sender<RoutingAgentRequest> {
        let (channel_sender, channel_receiver) = mpsc::channel::<RoutingAgentRequest>(1);
        self.channel_receiver = Some(channel_receiver);
        return channel_sender;
    }

    fn message_add_route(&mut self, target: Endpoint, route_type: RouteType, next_hop: Endpoint) {
        self.routes
            .entry(target)
            .or_insert_with(|| HashSet::new())
            .insert(RouteEntry {
                route_type,
                next_hop,
            });
    }

    fn message_remove_route(
        &mut self,
        target: Endpoint,
        route_type: RouteType,
        next_hop: Endpoint,
    ) {
        self.routes
            .entry(target)
            .or_insert_with(|| HashSet::new())
            .remove(&RouteEntry {
                route_type,
                next_hop,
            });
    }

    fn message_get_next_hop(
        &mut self,
        target: Endpoint,
        responder: oneshot::Sender<Option<Endpoint>>,
    ) {
        let response = self.routes.get(&target.get_node_endpoint()).and_then(|s| {
            let mut v = s.into_iter().collect::<Vec<&RouteEntry>>();
            v.sort_unstable_by_key(|e| e.route_type);
            if v.len() == 0 {
                None
            } else {
                Some(v[0].next_hop.clone())
            }
        });
        if let Err(e) = responder.send(response) {
            error!("Error sending response for getting next hop: {:?}", e);
        }
    }
}
