use actix::prelude::*;
use bp7::{bundle::Bundle, endpoint::Endpoint};
use tokio::sync::oneshot;

use crate::bundlestorageagent::StoredBundle;


#[derive(Message)]
#[rtype(result = "")]
pub struct NewRoutesAvailable {
    destinations: Vec<Endpoint>,
}

#[derive(Message)]
#[rtype(result = "")]
pub struct ReceiveBundle {
    bundle: Bundle,
    responder: oneshot::Sender<Result<(), ()>>,
}

#[derive(Message)]
#[rtype(result = "")]
pub struct ForwardBundleResult {
    result: Result<(), ()>,
    bundle: StoredBundle,
}
