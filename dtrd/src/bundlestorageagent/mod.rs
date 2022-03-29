use std::sync::Arc;

use bp7::bundle::Bundle;
use uuid::Uuid;

pub mod agent;
pub mod messages;
pub mod client;

#[derive(Debug, Eq)]
pub struct StoredBundle {
    bundle: Arc<Bundle>,
    id: Uuid,
}

impl StoredBundle {
    pub fn get_bundle(&self) -> &Bundle {
        return &self.bundle;
    }
}

impl Clone for StoredBundle {
    fn clone(&self) -> Self {
        Self {
            bundle: self.bundle.clone(),
            id: self.id,
        }
    }
}

impl PartialEq for StoredBundle {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl PartialEq<StoredBundle> for &StoredBundle {
    fn eq(&self, other: &StoredBundle) -> bool {
        self.id == other.id
    }
}

impl From<Bundle> for StoredBundle {
    fn from(bundle: Bundle) -> Self {
        Self {
            bundle: Arc::new(bundle),
            id: Uuid::new_v4(),
        }
    }
}
