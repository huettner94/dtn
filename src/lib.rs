use serde::{Deserialize, Serialize};

pub mod blockflags;
pub mod bundle;
pub mod bundleflags;
pub mod crc;
pub mod endpoint;
pub mod primaryblock;

pub trait Validate {
    fn validate(&self) -> bool;
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CreationTimestamp {
    pub creation_time: DtnTime,
    pub sequence_number: u64,
}

type DtnTime = u64;
