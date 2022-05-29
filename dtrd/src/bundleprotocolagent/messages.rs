use bp7::{bundle::Bundle, endpoint::Endpoint};
use tokio::sync::oneshot;

use crate::bundlestorageagent::StoredBundle;

#[derive(Debug)]
pub enum BPARequest {
    SendBundle {
        destination: Endpoint,
        payload: Vec<u8>,
        lifetime: u64,
        responder: oneshot::Sender<Result<(), ()>>,
    },
    IsEndpointLocal {
        endpoint: Endpoint,
        sender: oneshot::Sender<bool>,
    },
    NewClientConnected {
        destination: Endpoint,
    },
    NewRoutesAvailable {
        destinations: Vec<Endpoint>,
    },
    ReceiveBundle {
        bundle: Bundle,
        responder: oneshot::Sender<Result<(), ()>>,
    },
    ForwardBundleResult {
        result: Result<(), ()>,
        bundle: StoredBundle,
    },
}
