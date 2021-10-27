use std::fmt::Display;

use bp7::endpoint::Endpoint;
use tokio::sync::oneshot;

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum NodeConnectionStatus {
    Disconnected,
    Connected,
    Connecting,
}

impl Display for NodeConnectionStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            NodeConnectionStatus::Disconnected => "Disconnected",
            NodeConnectionStatus::Connected => "Connected",
            NodeConnectionStatus::Connecting => "Connecting",
        })
    }
}

#[derive(Debug, Clone, Eq)]
pub struct Node {
    pub url: String,
    pub connection_status: NodeConnectionStatus,
    pub remote_endpoint: Option<Endpoint>,
    pub temporary: bool,
}

impl PartialEq for Node {
    fn eq(&self, other: &Self) -> bool {
        self.url == other.url
    }
}

#[derive(Debug)]
pub enum NodeAgentRequest {
    ListNodes {
        responder: oneshot::Sender<Vec<Node>>,
    },
    AddNode {
        url: String,
    },
    RemoveNode {
        url: String,
    },
    NotifyNodeConnected {
        url: String,
        endpoint: Endpoint,
    },
    NotifyNodeDisconnected {
        url: String,
    },
}
