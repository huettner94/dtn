use bp7::{bundle::Bundle, endpoint::Endpoint};
use tokio::sync::{mpsc, oneshot};

use crate::bundlestorageagent::StoredBundle;

#[derive(Debug)]
pub struct AgentForwardBundle {
    pub bundle: StoredBundle,
    pub responder: oneshot::Sender<Result<(), ()>>,
}

#[derive(Debug)]
pub enum ConverganceAgentRequest {
    AgentGetNode {
        destination: Endpoint,
        responder: oneshot::Sender<Option<mpsc::Sender<AgentForwardBundle>>>,
    },
    AgentConnectNode {
        connection_string: String,
    },
    AgentDisconnectNode {
        connection_string: String,
    },
    CLRegisterNode {
        url: String,
        node: Endpoint,
        max_bundle_size: u64,
        sender: mpsc::Sender<AgentForwardBundle>,
    },
    CLUnregisterNode {
        url: String,
        node: Option<Endpoint>,
    },
    CLForwardBundle {
        bundle: Bundle,
        responder: oneshot::Sender<Result<(), ()>>
    },
}
