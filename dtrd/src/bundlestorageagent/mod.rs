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

use std::sync::{Arc, Weak};

use bp7::{bundle::Bundle, primaryblock::PrimaryBlock};

pub mod agent;
pub mod messages;

#[derive(Debug, Copy, Clone)]
pub enum State {
    /// Received from a remote node, or from a local connection.
    /// Bundle is stored, but not processed in any way.
    Received,
    /// Bundle is valid (e.g. structure + crc match),
    /// "received" status report has been sent.
    Valid,
    /// Bundle is ready for local delivery and awaiting sending
    /// to a local connection.
    DeliveryQueued,
    /// Bundle has been locally delivered and "delivered" status
    /// report has been sent. Bundle may now be deleted.
    Delivered,
    /// Bundle is ready for forwarding and awaiting sending to
    /// some remote. Bundle adjustments like "hop count" have
    /// been made.
    ForwardingQueued,
    /// Bundle has been forwarded and "forwarded" status report has
    /// been sent. Bundle may now be deleted.
    Forwarded,
    /// Bundle is for some reason invalid and will not be further processed.
    /// Bundle may now be deleted.
    Invalid,
}

#[derive(Debug)]
pub struct StoredBundle {
    bundle_data: Arc<Vec<u8>>,
    state: State,
    size: u64,
    min_size: Option<u64>,
    primary_block: PrimaryBlock,
}

fn id_from_pb(pb: &PrimaryBlock) -> String {
    format!(
        "{}:{}:{}:{}",
        pb.source_node,
        pb.creation_timestamp.creation_time.timestamp,
        pb.creation_timestamp.sequence_number,
        pb.fragment_offset.unwrap_or_default()
    )
}

impl StoredBundle {
    pub fn get_id(&self) -> String {
        id_from_pb(&self.primary_block)
    }

    pub fn get_filename(&self) -> String {
        id_from_pb(&self.primary_block).replace('/', "_")
    }

    pub fn get_bundle(&self) -> Bundle<'_> {
        self.bundle_data.as_slice().try_into().unwrap()
    }

    pub fn get_state(&self) -> State {
        self.state
    }

    pub fn get_bundle_size(&self) -> u64 {
        self.size
    }

    pub fn get_bundle_min_size(&self) -> Option<u64> {
        self.min_size
    }

    pub fn get_primary_block(&self) -> &PrimaryBlock {
        &self.primary_block
    }

    fn get_ref(&self) -> StoredBundleRef {
        StoredBundleRef {
            bundle_data: Arc::downgrade(&self.bundle_data),
            state: self.state,
            size: self.size,
            min_size: self.min_size,
            primary_block: self.primary_block.clone(),
        }
    }
}

impl PartialEq for StoredBundle {
    fn eq(&self, other: &Self) -> bool {
        self.primary_block == other.primary_block
    }
}

impl PartialEq<StoredBundle> for &StoredBundle {
    fn eq(&self, other: &StoredBundle) -> bool {
        self.primary_block == other.primary_block
    }
}

impl From<Vec<u8>> for StoredBundle {
    fn from(bundle_data: Vec<u8>) -> Self {
        let bundle: Bundle = bundle_data.as_slice().try_into().unwrap();
        let primary_block = bundle.primary_block.clone();
        let size = bundle_data.len() as u64;
        Self {
            bundle_data: Arc::new(bundle_data),
            state: State::Received,
            size,
            min_size: None,
            primary_block,
        }
    }
}

#[derive(Debug, Clone)]
pub struct StoredBundleRef {
    bundle_data: Weak<Vec<u8>>,
    state: State,
    size: u64,
    min_size: Option<u64>,
    primary_block: PrimaryBlock,
}

impl StoredBundleRef {
    pub fn get_id(&self) -> String {
        id_from_pb(&self.primary_block)
    }

    pub fn get_bundle_data(&self) -> Option<Arc<Vec<u8>>> {
        self.bundle_data.upgrade()
    }

    pub fn get_state(&self) -> State {
        self.state
    }

    pub fn get_bundle_size(&self) -> u64 {
        self.size
    }

    pub fn get_bundle_min_size(&self) -> Option<u64> {
        self.min_size
    }

    pub fn get_primary_block(&self) -> &PrimaryBlock {
        &self.primary_block
    }
}

impl PartialEq for StoredBundleRef {
    fn eq(&self, other: &Self) -> bool {
        self.primary_block == other.primary_block
    }
}

impl PartialEq<StoredBundleRef> for &StoredBundleRef {
    fn eq(&self, other: &StoredBundleRef) -> bool {
        self.primary_block == other.primary_block
    }
}

impl PartialEq<StoredBundleRef> for &StoredBundle {
    fn eq(&self, other: &StoredBundleRef) -> bool {
        self.primary_block == other.primary_block
    }
}
