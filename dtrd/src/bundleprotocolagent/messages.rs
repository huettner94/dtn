use actix::prelude::*;
use bp7::{bundle::Bundle, endpoint::Endpoint};

use crate::bundlestorageagent::StoredBundle;

#[derive(Message)]
#[rtype(result = "Result<(), ()>")]
pub struct SendBundle {
    destination: Endpoint,
    payload: Vec<u8>,
    lifetime: u64,
}

#[derive(Message)]
#[rtype(result = "")]
pub struct NewClientConnected {
    destination: Endpoint,
}

#[derive(Message)]
#[rtype(result = "")]
pub struct NewRoutesAvailable {
    destinations: Vec<Endpoint>,
}

#[derive(Message)]
#[rtype(result = "Result<(), ()>")]
pub struct ReceiveBundle {
    bundle: Bundle,
}

#[derive(Message)]
#[rtype(result = "")]
pub struct ForwardBundleResult {
    result: Result<(), ()>,
    bundle: StoredBundle,
}
