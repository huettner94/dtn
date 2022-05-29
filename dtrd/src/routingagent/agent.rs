use std::{
    collections::{HashMap, HashSet},
    hash::Hash,
};

use async_trait::async_trait;
use bp7::endpoint::Endpoint;
use log::{debug, error, warn};
use tokio::sync::{mpsc, oneshot};

use crate::{bundleprotocolagent::messages::BPARequest, common::settings::Settings};

use super::messages::{NexthopInfo, RouteStatus, RouteType, RoutingAgentRequest};

#[derive(Debug, Eq)]
struct RouteEntry {
    route_type: RouteType,
    next_hop: Endpoint,
    max_bundle_size: Option<u64>,
}

impl Hash for RouteEntry {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.route_type.hash(state);
        self.next_hop.hash(state);
    }
}

impl PartialEq for RouteEntry {
    fn eq(&self, other: &Self) -> bool {
        self.route_type == other.route_type && self.next_hop == other.next_hop
    }
}

pub struct Daemon {
    routes: HashMap<Endpoint, HashSet<RouteEntry>>,
    channel_receiver: Option<mpsc::UnboundedReceiver<RoutingAgentRequest>>,
    bpa_sender: Option<mpsc::UnboundedSender<BPARequest>>,
}

#[async_trait]
impl crate::common::agent::Daemon for Daemon {
    type MessageType = RoutingAgentRequest;

    async fn new(_: &Settings) -> Self {
        Daemon {
            routes: HashMap::new(),
            channel_receiver: None,
            bpa_sender: None,
        }
    }

    fn get_agent_name(&self) -> &'static str {
        "Routing Agent"
    }

    fn get_channel_receiver(&mut self) -> Option<mpsc::UnboundedReceiver<Self::MessageType>> {
        self.channel_receiver.take()
    }

    fn validate(&self) {
        if self.bpa_sender.is_none() {
            panic!("Must call set_senders before calling run (also run may only be called once)");
        }
    }

    async fn handle_message(&mut self, msg: RoutingAgentRequest) {
        match msg {
            RoutingAgentRequest::AddRoute {
                target,
                route_type,
                next_hop,
                max_bundle_size,
            } => {
                self.message_add_route(target, route_type, next_hop, max_bundle_size)
                    .await
            }
            RoutingAgentRequest::RemoveRoute {
                target,
                route_type,
                next_hop,
            } => {
                self.message_remove_route(target, route_type, next_hop)
                    .await
            }
            RoutingAgentRequest::GetNextHop { target, responder } => {
                self.message_get_next_hop(target, responder).await
            }
            RoutingAgentRequest::ListRoutes { responder } => {
                self.message_list_routes(responder).await
            }
        }
    }
}

impl Daemon {
    pub fn init_channels(&mut self) -> mpsc::UnboundedSender<RoutingAgentRequest> {
        let (channel_sender, channel_receiver) = mpsc::unbounded_channel::<RoutingAgentRequest>();
        self.channel_receiver = Some(channel_receiver);
        return channel_sender;
    }

    pub fn set_senders(&mut self, bpa_sender: mpsc::UnboundedSender<BPARequest>) {
        self.bpa_sender = Some(bpa_sender);
    }

    async fn message_add_route(
        &mut self,
        target: Endpoint,
        route_type: RouteType,
        next_hop: Endpoint,
        max_bundle_size: Option<u64>,
    ) {
        let prev_routes = self.get_available_routes();
        if self
            .routes
            .entry(target.clone())
            .or_insert_with(|| HashSet::new())
            .insert(RouteEntry {
                route_type,
                next_hop,
                max_bundle_size,
            })
        {
            let available_routes = self.get_available_routes();
            let new_routes: Vec<Endpoint> = available_routes
                .difference(&prev_routes)
                .map(|e| e.clone())
                .collect();
            debug!("New routes added to routing table for: {:?}", new_routes);
            if !new_routes.is_empty() {
                if let Err(e) =
                    self.bpa_sender
                        .as_ref()
                        .unwrap()
                        .send(BPARequest::NewRoutesAvailable {
                            destinations: new_routes,
                        })
                {
                    error!("Error sending new route notification to bpa: {:?}", e);
                }
            }
        }
    }

    async fn message_remove_route(
        &mut self,
        target: Endpoint,
        route_type: RouteType,
        next_hop: Endpoint,
    ) {
        let endpoint_routes = self
            .routes
            .entry(target.clone())
            .or_insert_with(|| HashSet::new());
        let entry_to_remove = RouteEntry {
            route_type,
            next_hop: next_hop.clone(),
            max_bundle_size: None, // irrelevant as this is not part of Eq
        };
        match endpoint_routes.remove(&entry_to_remove) {
            true => debug!(
                "Removed route for {} over {} from routing table",
                target, next_hop
            ),
            false => warn!("No route found to remove for {} over {}", target, next_hop),
        }
    }

    async fn message_get_next_hop(
        &self,
        target: Endpoint,
        responder: oneshot::Sender<Option<NexthopInfo>>,
    ) {
        let response = self.routes.get(&target.get_node_endpoint()).and_then(|s| {
            let mut v = s
                .into_iter()
                .filter(|r| {
                    if r.route_type == RouteType::Connected {
                        true
                    } else {
                        self.routes
                            .get(&r.next_hop)
                            .and_then(|s| {
                                Some(
                                    s.into_iter()
                                        .any(|re| re.route_type == RouteType::Connected),
                                )
                            })
                            .unwrap_or(false)
                    }
                })
                .collect::<Vec<&RouteEntry>>();
            v.sort_unstable_by_key(|e| e.route_type);
            if v.len() == 0 {
                None
            } else {
                Some(NexthopInfo {
                    next_hop: v[0].next_hop.clone(),
                    max_size: v[0].max_bundle_size,
                })
            }
        });
        if let Err(e) = responder.send(response) {
            error!("Error sending response for getting next hop: {:?}", e);
        }
    }

    async fn message_list_routes(&self, responder: oneshot::Sender<Vec<RouteStatus>>) {
        let connected_routes = self.get_connected_routes();

        let routes: Vec<RouteStatus> = self
            .routes
            .iter()
            .map(|(target, routes)| {
                let mut routes: Vec<RouteStatus> = routes
                    .into_iter()
                    .map(|r| {
                        let available = r.route_type == RouteType::Connected
                            || connected_routes.contains(&r.next_hop);
                        RouteStatus {
                            target: target.clone(),
                            next_hop: r.next_hop.clone(),
                            available,
                            preferred: false,
                            route_type: r.route_type,
                            max_bundle_size: r.max_bundle_size,
                        }
                    })
                    .collect();
                routes.sort_unstable_by_key(|e| e.route_type);
                if routes.len() != 0 && routes[0].available {
                    routes[0].preferred = true;
                }
                routes
            })
            .flatten()
            .collect();
        if let Err(e) = responder.send(routes) {
            error!("Error sending response for listing routes: {:?}", e);
        }
    }

    fn get_available_routes(&self) -> HashSet<Endpoint> {
        let mut connected_routes = self.get_connected_routes();

        let other_routes: HashSet<Endpoint> = self
            .routes
            .iter()
            .filter_map(|(target, routes)| {
                if routes.into_iter().any(|r| {
                    r.route_type != RouteType::Connected && connected_routes.contains(&r.next_hop)
                }) {
                    Some(target.clone())
                } else {
                    None
                }
            })
            .collect();
        connected_routes.extend(other_routes);
        connected_routes
    }

    fn get_connected_routes(&self) -> HashSet<Endpoint> {
        self.routes
            .iter()
            .filter_map(|(target, routes)| {
                if routes
                    .into_iter()
                    .any(|r| r.route_type == RouteType::Connected)
                {
                    Some(target.clone())
                } else {
                    None
                }
            })
            .collect()
    }
}
