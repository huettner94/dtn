// Copyright (C) 2023 Felix Huettner
//
// This file is part of DTRD.
//
// DTRD is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// DTRD is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with this program.  If not, see <https://www.gnu.org/licenses/>.

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
