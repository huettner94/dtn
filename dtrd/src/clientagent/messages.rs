use crate::{
    bundlestorageagent::StoredBundle, nodeagent::messages::Node,
    routingagent::messages::RouteStatus,
};
use actix::prelude::*;
use bp7::endpoint::Endpoint;

#[derive(Message)]
#[rtype(result = "")]
pub struct ClientDeliverBundle {
    bundle: StoredBundle,
    responder: Recipient<EventBundleDelivered>,
}

#[derive(Message)]
#[rtype(result = "")]
pub struct EventBundleDelivered {
    endpoint: Endpoint,
    bundle: StoredBundle,
}

#[derive(Message)]
#[rtype(result = "")]
pub struct EventClientConnected {
    destination: Endpoint,
    sender: Recipient<ClientDeliverBundle>,
}
#[derive(Message)]
#[rtype(result = "")]
pub struct EventClientDisconnected {
    destination: Endpoint,
}

#[derive(Message)]
#[rtype(result = "Result<(), ()>")]
pub struct ClientSendBundle {
    destination: Endpoint,
    payload: Vec<u8>,
    lifetime: u64,
}
#[derive(Message)]
#[rtype(result = "Vec<Node>")]
pub struct ClientListNodes {}

#[derive(Message)]
#[rtype(result = "")]
pub struct ClientAddNode {
    url: String,
}

#[derive(Message)]
#[rtype(result = "")]
pub struct ClientRemoveNode {
    url: String,
}

#[derive(Message)]
#[rtype(result = "Vec<RouteStatus>")]
pub struct ClientListRoutes {}

#[derive(Message)]
#[rtype(result = "")]
pub struct ClientAddRoute {
    target: Endpoint,
    next_hop: Endpoint,
}

#[derive(Message)]
#[rtype(result = "")]
pub struct ClientRemoveRoute {
    target: Endpoint,
    next_hop: Endpoint,
}
