use bp7::endpoint::Endpoint;
use tokio::sync::oneshot;

#[derive(Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Clone, Copy)]
pub enum RouteType {
    Connected = 0,
    Static = 1,
}

#[derive(Debug)]
pub enum RoutingAgentRequest {
    AddRoute {
        target: Endpoint,
        route_type: RouteType,
        next_hop: Endpoint,
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
}
