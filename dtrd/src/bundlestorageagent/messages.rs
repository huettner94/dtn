use bp7::{bundle::Bundle, endpoint::Endpoint};
use tokio::sync::oneshot;

#[derive(Debug)]
pub enum BSARequest {
    StoreBundle {
        bundle: Bundle,
    },
    GetBundleForDestination {
        destination: Endpoint,
        bundles: oneshot::Sender<Result<Vec<Bundle>, String>>,
    },
    GetBundleForNode {
        destination: Endpoint,
        bundles: oneshot::Sender<Result<Vec<Bundle>, String>>,
    },
}
