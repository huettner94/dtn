use actix::prelude::*;
use bp7::endpoint::Endpoint;
use url::Url;

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
pub struct EventBundleForwardingFailed {
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
    pub url: Url,
}
#[derive(Message)]
#[rtype(result = "")]
pub struct AgentDisconnectNode {
    pub url: Url,
}
#[derive(Message)]
#[rtype(result = "")]
pub struct CLRegisterNode {
    pub url: Url,
    pub node: Endpoint,
    pub max_bundle_size: u64,
    pub sender: Recipient<AgentForwardBundle>,
}
#[derive(Message)]
#[rtype(result = "")]
pub struct CLUnregisterNode {
    pub url: Url,
    pub node: Option<Endpoint>,
}
