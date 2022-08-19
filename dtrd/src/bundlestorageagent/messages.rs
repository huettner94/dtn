use bp7::{bundle::Bundle, endpoint::Endpoint};

use super::StoredBundle;
use actix::prelude::*;

#[derive(Message)]
#[rtype(result = "")]
pub struct EventNewBundleStored {
    bundle: StoredBundle,
}

#[derive(Message)]
#[rtype(result = "Result<(), ()>")]
pub struct StoreBundle {
    bundle: Bundle,
}

#[derive(Message)]
#[rtype(result = "Result<(), ()>")]
pub struct StoreNewBundle {
    bundle: Bundle,
}

#[derive(Message)]
#[rtype(result = "")]
pub struct DeleteBundle {
    bundle: StoredBundle,
}

#[derive(Message)]
#[rtype(result = "Result<Vec<StoredBundle>, String>")]
pub struct GetBundleForDestination {
    destination: Endpoint,
}

#[derive(Message)]
#[rtype(result = "Result<Vec<StoredBundle>, String>")]
pub struct GetBundleForNode {
    destination: Endpoint,
}