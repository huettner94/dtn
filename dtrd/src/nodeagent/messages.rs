use std::fmt::Display;

use actix::prelude::*;
use bp7::endpoint::Endpoint;
use url::Url;

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum NodeConnectionStatus {
    Disconnected,
    Connected,
    Connecting,
    Disconnecting,
}

impl Display for NodeConnectionStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            NodeConnectionStatus::Disconnected => "Disconnected",
            NodeConnectionStatus::Connected => "Connected",
            NodeConnectionStatus::Connecting => "Connecting",
            NodeConnectionStatus::Disconnecting => "Disconnecting",
        })
    }
}
#[derive(Debug, Clone, Eq)]
pub struct Node {
    pub url: Url,
    pub connection_status: NodeConnectionStatus,
    pub remote_endpoint: Option<Endpoint>,
    pub temporary: bool,
}

impl PartialEq for Node {
    fn eq(&self, other: &Self) -> bool {
        self.url == other.url
    }
}

#[derive(Message)]
#[rtype(result = "Vec<Node>")]
pub struct ListNodes {}

#[derive(Message)]
#[rtype(result = "")]
pub struct AddNode {
    pub url: Url,
}

#[derive(Message)]
#[rtype(result = "")]
pub struct RemoveNode {
    pub url: Url,
}

#[derive(Message)]
#[rtype(result = "")]
pub struct NotifyNodeConnected {
    pub url: Url,
    pub endpoint: Endpoint,
    pub max_bundle_size: u64,
}

#[derive(Message)]
#[rtype(result = "")]
pub struct NotifyNodeDisconnected {
    pub url: Url,
}

#[derive(Message)]
#[rtype(result = "")]
pub struct TryConnect {}
