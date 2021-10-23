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
    CLRegisterNode {
        node: Endpoint,
        sender: mpsc::Sender<AgentForwardBundle>,
    },
    CLUnregisterNode {
        node: Endpoint,
    },
    CLForwardBundle {
        bundle: Bundle,
    },
}
