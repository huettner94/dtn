use std::collections::HashMap;

use actix::prelude::*;
use bp7::endpoint::Endpoint;

#[derive(Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Clone, Copy)]
pub enum RouteType {
    Connected = 0,
    Static = 1,
}

#[derive(Debug, PartialEq, Eq)]
pub struct RouteStatus {
    pub target: Endpoint,
    pub next_hop: Endpoint,
    pub route_type: RouteType,
    pub preferred: bool,
    pub available: bool,
    pub max_bundle_size: Option<u64>,
}

#[derive(Debug, PartialEq, Eq)]
pub struct NexthopInfo {
    pub next_hop: Endpoint,
    pub max_size: Option<u64>,
}

#[derive(Message)]
#[rtype(result = "")]
pub struct EventRoutingTableUpdate {
    pub routes: HashMap<Endpoint, NexthopInfo>,
}

#[derive(Message)]
#[rtype(result = "")]
pub struct AddRoute {
    pub target: Endpoint,
    pub route_type: RouteType,
    pub next_hop: Endpoint,
    pub max_bundle_size: Option<u64>,
}

#[derive(Message)]
#[rtype(result = "")]
pub struct RemoveRoute {
    pub target: Endpoint,
    pub route_type: RouteType,
    pub next_hop: Endpoint,
}

#[derive(Message)]
#[rtype(result = "Vec<RouteStatus>")]
pub struct ListRoutes {}
