use bp7::{bundle::Bundle, endpoint::Endpoint};
use tokio::sync::oneshot;

use super::StoredBundle;

#[derive(Debug)]
pub enum BSARequest {
    StoreBundle {
        bundle: Bundle,
        responder: oneshot::Sender<Result<StoredBundle, ()>>,
    },
    DeleteBundle {
        bundle: StoredBundle,
    },
    GetBundleForDestination {
        destination: Endpoint,
        bundles: oneshot::Sender<Result<Vec<StoredBundle>, String>>,
    },
    GetBundleForNode {
        destination: Endpoint,
        bundles: oneshot::Sender<Result<Vec<StoredBundle>, String>>,
    },
    TryDefragmentBundle {
        bundle: StoredBundle,
        responder: oneshot::Sender<Result<Option<StoredBundle>, ()>>,
    },
}
