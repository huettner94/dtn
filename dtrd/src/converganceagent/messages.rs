use bp7::{bundle::Bundle, endpoint::Endpoint};
use tokio::sync::{mpsc, oneshot};

#[derive(Debug)]
pub struct AgentForwardBundle {
    pub bundle: Bundle,
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
        sender: mpsc::Sender<AgentForwardBundle>,
    },
    CLUnregisterNode {
        url: String,
        node: Option<Endpoint>,
    },
    CLForwardBundle {
        bundle: Bundle,
    },
}
