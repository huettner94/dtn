use bitflags::bitflags;
use serde::{Deserialize, Serialize};

use crate::Validate;

bitflags! {
    #[derive(Serialize, Deserialize)]
    pub struct BlockFlags: u64 {
        const MUST_REPLICATE_TO_ALL_FRAGMENTS = 0x01;
        const STATUS_REPORT_REQUESTED_WHEN_NOT_PROCESSABLE = 0x02;
        const DELETE_BUNDLE_WHEN_NOT_PROCESSABLE = 0x04;
        const DELETE_BLOCK_WHEN_NOT_PROCESSABLE = 0x10;
    }
}

impl Validate for BlockFlags {
    fn validate(&self) -> bool {
        return true;
    }
}
