use bp7::endpoint::Endpoint;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use crate::{nodeagent::messages::Node, routingagent::messages::RouteStatus};
use actix::prelude::*;

#[derive(Debug)]
pub struct ListenBundlesResponse {
    pub endpoint: Endpoint,
    pub data: Vec<u8>,
}

#[derive(Message)]
#[rtype(result = "Result<(), ()>")]
pub struct ClientSendBundle {
    destination: Endpoint,
    payload: Vec<u8>,
    lifetime: u64,
}
#[derive(Message)]
#[rtype(result = "Result<(), String>")]
pub struct ClientListenBundles {
    destination: Endpoint,
    responder: mpsc::Sender<ListenBundlesResponse>,
    canceltoken: CancellationToken,
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

#[derive(Message)]
#[rtype(result = "Option<mpsc::Sender<ListenBundlesResponse>>")]
pub struct AgentGetClient {
    destination: Endpoint,
}
