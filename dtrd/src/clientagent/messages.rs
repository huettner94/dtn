use crate::{
    bundlestorageagent::StoredBundle, nodeagent::messages::Node,
    routingagent::messages::RouteStatus,
};
use actix::prelude::*;
use bp7::endpoint::Endpoint;

#[derive(Message)]
#[rtype(result = "")]
pub struct ClientDeliverBundle {
    pub bundle: StoredBundle,
    pub responder: Recipient<EventBundleDelivered>,
}

#[derive(Message)]
#[rtype(result = "")]
pub struct EventBundleDelivered {
    pub endpoint: Endpoint,
    pub bundle: StoredBundle,
}

#[derive(Message)]
#[rtype(result = "")]
pub struct EventClientConnected {
    pub destination: Endpoint,
    pub sender: Recipient<ClientDeliverBundle>,
}

#[derive(Message)]
#[rtype(result = "")]
pub struct EventClientDisconnected {
    pub destination: Endpoint,
}

#[derive(Message)]
#[rtype(result = "Result<(), ()>")]
pub struct ClientSendBundle {
    pub destination: Endpoint,
    pub payload: Vec<u8>,
    pub lifetime: u64,
}
#[derive(Message)]
#[rtype(result = "Vec<Node>")]
pub struct ClientListNodes {}

#[derive(Message)]
#[rtype(result = "")]
pub struct ClientAddNode {
    pub url: String,
}

#[derive(Message)]
#[rtype(result = "")]
pub struct ClientRemoveNode {
    pub url: String,
}

#[derive(Message)]
#[rtype(result = "Vec<RouteStatus>")]
pub struct ClientListRoutes {}

#[derive(Message)]
#[rtype(result = "")]
pub struct ClientAddRoute {
    pub target: Endpoint,
    pub next_hop: Endpoint,
}

#[derive(Message)]
#[rtype(result = "")]
pub struct ClientRemoveRoute {
    pub target: Endpoint,
    pub next_hop: Endpoint,
}
