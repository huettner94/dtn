use std::sync::Arc;

use bp7::bundle::Bundle;
use uuid::Uuid;

pub mod agent;
pub mod messages;

#[derive(Debug, Eq)]
pub struct StoredBundle {
    bundle: Arc<Bundle>,
    id: Uuid,
    size: u64,
    min_size: Option<u64>,
}

impl StoredBundle {
    pub fn get_id(&self) -> Uuid {
        self.id
    }
    pub fn get_bundle(&self) -> &Bundle {
        &self.bundle
    }

    pub fn get_bundle_size(&self) -> u64 {
        self.size
    }

    pub fn get_bundle_min_size(&self) -> Option<u64> {
        self.min_size
    }
}

impl Clone for StoredBundle {
    fn clone(&self) -> Self {
        Self {
            bundle: self.bundle.clone(),
            id: self.id,
            size: self.size,
            min_size: self.min_size,
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

impl TryFrom<Bundle> for StoredBundle {
    type Error = bp7::SerializationError;

    fn try_from(bundle: Bundle) -> Result<Self, Self::Error> {
        let bundle_as_bytes: Vec<u8> = bundle.clone().try_into()?; // TODO: This is bad because of a full clone
        let size = bundle_as_bytes.len() as u64;
        Ok(Self {
            bundle: Arc::new(bundle),
            id: Uuid::new_v4(),
            size,
            min_size: None,
        })
    }
}
