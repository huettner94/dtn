use actix::prelude::*;
use bp7::endpoint::Endpoint;

#[derive(Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Clone, Copy)]
pub enum RouteType {
    Connected = 0,
    Static = 1,
}

#[derive(Debug)]
pub struct RouteStatus {
    pub target: Endpoint,
    pub next_hop: Endpoint,
    pub route_type: RouteType,
    pub preferred: bool,
    pub available: bool,
    pub max_bundle_size: Option<u64>,
}

#[derive(Debug)]
pub struct NexthopInfo {
    pub next_hop: Endpoint,
    pub max_size: Option<u64>,
}

#[derive(Message)]
#[rtype(result = "")]
pub struct AddRoute {
    target: Endpoint,
    route_type: RouteType,
    next_hop: Endpoint,
    max_bundle_size: Option<u64>,
}

#[derive(Message)]
#[rtype(result = "")]
pub struct RemoveRoute {
    target: Endpoint,
    route_type: RouteType,
    next_hop: Endpoint,
}

#[derive(Message)]
#[rtype(result = "Option<NexthopInfo>")]
pub struct GetNextHop {
    target: Endpoint,
}

#[derive(Message)]
#[rtype(result = "Vec<RouteStatus>")]
pub struct ListRoutes {}
