use actix::prelude::*;
use bp7::{bundle::Bundle, endpoint::Endpoint};
use tokio::sync::oneshot;

use crate::bundlestorageagent::StoredBundle;

#[derive(Message)]
#[rtype(result = "")]
pub struct AgentForwardBundle {
    pub bundle: StoredBundle,
    pub responder: Recipient<EventBundleForwarded>,
}

#[derive(Message)]
#[rtype(result = "")]
pub struct EventBundleForwarded {
    pub endpoint: Endpoint,
    pub bundle: StoredBundle,
}

#[derive(Message)]
#[rtype(result = "")]
pub struct EventPeerConnected {
    pub destination: Endpoint,
    pub sender: Recipient<AgentForwardBundle>,
}

#[derive(Message)]
#[rtype(result = "")]
pub struct EventPeerDisconnected {
    pub destination: Endpoint,
}

#[derive(Message)]
#[rtype(result = "")]
pub struct AgentConnectNode {
    pub connection_string: String,
}
#[derive(Message)]
#[rtype(result = "")]
pub struct AgentDisconnectNode {
    pub connection_string: String,
}
#[derive(Message)]
#[rtype(result = "")]
pub struct CLRegisterNode {
    pub url: String,
    pub node: Endpoint,
    pub max_bundle_size: u64,
    pub sender: Recipient<AgentForwardBundle>,
}
#[derive(Message)]
#[rtype(result = "")]
pub struct CLUnregisterNode {
    pub url: String,
    pub node: Option<Endpoint>,
}
#[derive(Message)]
#[rtype(result = "")]
pub struct CLForwardBundle {
    pub bundle: Bundle,
    pub responder: oneshot::Sender<Result<(), ()>>,
}
