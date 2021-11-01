use bp7::endpoint::Endpoint;
use tokio::sync::{mpsc, oneshot};
use tokio_util::sync::CancellationToken;

use crate::{nodeagent::messages::Node, routingagent::messages::RouteStatus};

#[derive(Debug)]
pub struct ListenBundlesResponse {
    pub endpoint: Endpoint,
    pub data: Vec<u8>,
}

#[derive(Debug)]
pub enum ClientAgentRequest {
    ClientSendBundle {
        destination: Endpoint,
        payload: Vec<u8>,
        lifetime: u64,
    },
    ClientListenBundles {
        destination: Endpoint,
        responder: mpsc::Sender<ListenBundlesResponse>,
        status: oneshot::Sender<Result<(), String>>,
        canceltoken: CancellationToken,
    },
    ClientListNodes {
        responder: oneshot::Sender<Vec<Node>>,
    },
    ClientAddNode {
        url: String,
    },
    ClientRemoveNode {
        url: String,
    },
    ClientListRoutes {
        responder: oneshot::Sender<Vec<RouteStatus>>,
    },
    ClientAddRoute {
        target: Endpoint,
        next_hop: Endpoint,
    },
    ClientRemoveRoute {
        target: Endpoint,
        next_hop: Endpoint,
    },
    AgentGetClient {
        destination: Endpoint,
        responder: oneshot::Sender<Option<mpsc::Sender<ListenBundlesResponse>>>,
    },
}
