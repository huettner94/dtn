use bp7::endpoint::Endpoint;
use tokio::sync::oneshot;

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
pub enum RoutingAgentRequest {
    AddRoute {
        target: Endpoint,
        route_type: RouteType,
        next_hop: Endpoint,
        max_bundle_size: Option<u64>,
    },
    RemoveRoute {
        target: Endpoint,
        route_type: RouteType,
        next_hop: Endpoint,
    },
    GetNextHop {
        target: Endpoint,
        responder: oneshot::Sender<Option<Endpoint>>,
    },
    ListRoutes {
        responder: oneshot::Sender<Vec<RouteStatus>>,
    },
}
