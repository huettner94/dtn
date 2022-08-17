use actix::prelude::*;
use bp7::{bundle::Bundle, endpoint::Endpoint};
use tokio::sync::{mpsc, oneshot};

use crate::bundlestorageagent::StoredBundle;

#[derive(Debug)]
pub struct AgentForwardBundle {
    pub bundle: StoredBundle,
    pub responder: oneshot::Sender<Result<(), ()>>,
}

#[derive(Message)]
#[rtype(result = "Option<mpsc::Sender<AgentForwardBundle>>")]
pub struct AgentGetNode {
    destination: Endpoint,
}
#[derive(Message)]
#[rtype(result = "")]
pub struct AgentConnectNode {
    connection_string: String,
}
#[derive(Message)]
#[rtype(result = "")]
pub struct AgentDisconnectNode {
    connection_string: String,
}
#[derive(Message)]
#[rtype(result = "")]
pub struct CLRegisterNode {
    url: String,
    node: Endpoint,
    max_bundle_size: u64,
    sender: mpsc::Sender<AgentForwardBundle>,
}
#[derive(Message)]
#[rtype(result = "")]
pub struct CLUnregisterNode {
    url: String,
    node: Option<Endpoint>,
}
#[derive(Message)]
#[rtype(result = "")]
pub struct CLForwardBundle {
    bundle: Bundle,
    responder: oneshot::Sender<Result<(), ()>>,
}
