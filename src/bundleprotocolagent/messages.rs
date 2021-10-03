use dtn::bp7::endpoint::Endpoint;
use tokio::sync::{mpsc, oneshot};

#[derive(Debug, PartialEq, Eq)]
pub struct ListenBundlesResponse {
    pub endpoint: Endpoint,
    pub data: Vec<u8>,
}

#[derive(Debug)]
pub enum BPARequest {
    SendBundle {
        destination: Endpoint,
        payload: Vec<u8>,
        lifetime: u64,
    },
    ListenBundles {
        destination: Endpoint,
        responder: mpsc::Sender<ListenBundlesResponse>,
        status: oneshot::Sender<Result<(), String>>,
    },
}
