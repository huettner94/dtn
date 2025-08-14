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

use actix::prelude::*;
use std::{
    collections::{HashMap, HashSet},
    hash::Hash,
};

use bp7::endpoint::Endpoint;
use log::{debug, warn};

use crate::routingagent::messages::EventRoutingTableUpdate;

use super::messages::{AddRoute, ListRoutes, NexthopInfo, RemoveRoute, RouteStatus, RouteType};

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
    last_routing_table: Option<HashMap<Endpoint, NexthopInfo>>,
}

impl Actor for Daemon {
    type Context = Context<Self>;
}

impl actix::Supervised for Daemon {}

impl SystemService for Daemon {}

impl Handler<AddRoute> for Daemon {
    type Result = ();

    fn handle(&mut self, msg: AddRoute, _ctx: &mut Context<Self>) -> Self::Result {
        let AddRoute {
            target,
            route_type,
            next_hop,
            max_bundle_size,
        } = msg;
        if self.routes.entry(target).or_default().insert(RouteEntry {
            route_type,
            next_hop,
            max_bundle_size,
        }) {
            self.send_route_update();
        }
    }
}

impl Handler<RemoveRoute> for Daemon {
    type Result = ();

    fn handle(&mut self, msg: RemoveRoute, _ctx: &mut Context<Self>) -> Self::Result {
        let RemoveRoute {
            target,
            route_type,
            next_hop,
        } = msg;
        let endpoint_routes = self.routes.entry(target.clone()).or_default();
        let entry_to_remove = RouteEntry {
            route_type,
            next_hop: next_hop.clone(),
            max_bundle_size: None, // irrelevant as this is not part of Eq
        };
        match endpoint_routes.remove(&entry_to_remove) {
            true => {
                if target == next_hop {
                    debug!("Removed direct route for {} from routing table", target)
                } else {
                    debug!(
                        "Removed route for {} via {} from routing table",
                        target, next_hop
                    );
                }
                self.send_route_update();
            }
            false => warn!("No route found to remove for {} over {}", target, next_hop),
        }
    }
}

impl Handler<ListRoutes> for Daemon {
    type Result = Vec<RouteStatus>;

    fn handle(&mut self, _msg: ListRoutes, _ctx: &mut Context<Self>) -> Self::Result {
        self.get_routes()
    }
}

impl Daemon {
    fn send_route_update(&self) {
        let routes: HashMap<Endpoint, NexthopInfo> = self
            .get_routes()
            .into_iter()
            .filter_map(|rs| match rs.preferred {
                true => Some((
                    rs.target,
                    NexthopInfo {
                        next_hop: rs.next_hop,
                        max_size: rs.max_bundle_size,
                    },
                )),
                false => None,
            })
            .collect();

        if let Some(lrt) = &self.last_routing_table
            && lrt == &routes
        {
            return;
        }

        debug!("Routing table changed, sending update.");
        crate::bundleprotocolagent::agent::Daemon::from_registry()
            .do_send(EventRoutingTableUpdate { routes });
    }

    fn get_routes(&self) -> Vec<RouteStatus> {
        let connected_routes = self.get_connected_routes();
        self.routes
            .iter()
            .flat_map(|(target, routes)| {
                let mut routes: Vec<RouteStatus> = routes
                    .iter()
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
                if !routes.is_empty() && routes[0].available {
                    routes[0].preferred = true;
                }
                routes
            })
            .collect()
    }

    fn get_connected_routes(&self) -> HashSet<Endpoint> {
        self.routes
            .iter()
            .filter_map(|(target, routes)| {
                if routes.iter().any(|r| r.route_type == RouteType::Connected) {
                    Some(target.clone())
                } else {
                    None
                }
            })
            .collect()
    }
}
