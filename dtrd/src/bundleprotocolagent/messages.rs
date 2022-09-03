use actix::prelude::*;
use bp7::{bundle::Bundle, endpoint::Endpoint};
use tokio::sync::oneshot;

use crate::bundlestorageagent::StoredBundle;

#[derive(Message)]
#[rtype(result = "")]
pub struct NewRoutesAvailable {
    pub destinations: Vec<Endpoint>,
}

#[derive(Message)]
#[rtype(result = "")]
pub struct ReceiveBundle {
    pub bundle: Bundle,
    pub responder: oneshot::Sender<Result<(), ()>>,
}

#[derive(Message)]
#[rtype(result = "")]
pub struct ForwardBundleResult {
    pub result: Result<(), ()>,
    pub bundle: StoredBundle,
}
