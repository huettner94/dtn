use actix::prelude::*;
use std::{
    collections::{HashMap, HashSet},
    hash::Hash,
};

use bp7::endpoint::Endpoint;
use log::{debug, warn};

use crate::bundleprotocolagent::messages::NewRoutesAvailable;

use super::messages::{
    AddRoute, GetNextHop, ListRoutes, NexthopInfo, RemoveRoute, RouteStatus, RouteType,
};

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

#[derive(Default)]
pub struct Daemon {
    routes: HashMap<Endpoint, HashSet<RouteEntry>>,
}

impl Actor for Daemon {
    type Context = Context<Self>;
}

impl actix::Supervised for Daemon {}

impl SystemService for Daemon {}

impl Handler<AddRoute> for Daemon {
    type Result = ();

    fn handle(&mut self, msg: AddRoute, ctx: &mut Context<Self>) -> Self::Result {
        let AddRoute {
            target,
            route_type,
            next_hop,
            max_bundle_size,
        } = msg;
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
                // TODO
                // crate::bundleprotocolagent::agent::Daemon::from_registry().do_send(
                //     NewRoutesAvailable {
                //         destinations: new_routes,
                //     },
                // );
            }
        }
    }
}

impl Handler<RemoveRoute> for Daemon {
    type Result = ();

    fn handle(&mut self, msg: RemoveRoute, ctx: &mut Context<Self>) -> Self::Result {
        let RemoveRoute {
            target,
            route_type,
            next_hop,
        } = msg;
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
}

impl Handler<GetNextHop> for Daemon {
    type Result = Option<NexthopInfo>;

    fn handle(&mut self, msg: GetNextHop, ctx: &mut Context<Self>) -> Self::Result {
        let GetNextHop { target } = msg;
        self.routes.get(&target.get_node_endpoint()).and_then(|s| {
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
        })
    }
}

impl Handler<ListRoutes> for Daemon {
    type Result = Vec<RouteStatus>;

    fn handle(&mut self, msg: ListRoutes, ctx: &mut Context<Self>) -> Self::Result {
        let connected_routes = self.get_connected_routes();
        self.routes
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
            .collect()
    }
}

impl Daemon {
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
