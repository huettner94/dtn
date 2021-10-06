use bp7::endpoint::Endpoint;
use tokio::sync::oneshot;

#[derive(Debug)]
pub enum BPARequest {
    SendBundle {
        destination: Endpoint,
        payload: Vec<u8>,
        lifetime: u64,
    },
    IsEndpointLocal {
        endpoint: Endpoint,
        sender: oneshot::Sender<bool>,
    },
    NewClientConnected {
        destination: Endpoint,
    },
}
