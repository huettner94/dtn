use bp7::endpoint::Endpoint;
use tokio::sync::{mpsc, oneshot};

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
    },
    AgentGetClient {
        destination: Endpoint,
        responder: oneshot::Sender<Option<mpsc::Sender<ListenBundlesResponse>>>,
    },
}
