use bp7::{bundle::Bundle, endpoint::Endpoint};

use super::StoredBundle;
use actix::prelude::*;

#[derive(Message)]
#[rtype(result = "")]
pub struct EventNewBundleStored {
    pub bundle: StoredBundle,
}

#[derive(Message)]
#[rtype(result = "Result<(), ()>")]
pub struct StoreBundle {
    pub bundle: Bundle,
}

#[derive(Message)]
#[rtype(result = "Result<(), ()>")]
pub struct StoreNewBundle {
    pub bundle: Bundle,
}

#[derive(Message)]
#[rtype(result = "")]
pub struct DeleteBundle {
    pub bundle: StoredBundle,
}

#[derive(Message)]
#[rtype(result = "Result<Vec<StoredBundle>, String>")]
pub struct GetBundleForDestination {
    pub destination: Endpoint,
}

#[derive(Message)]
#[rtype(result = "Result<Vec<StoredBundle>, String>")]
pub struct GetBundleForNode {
    pub destination: Endpoint,
}