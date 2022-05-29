use std::fmt::Debug;

use serde::Serialize;

use crate::Validate;

#[derive(PartialEq, Eq, Clone)]
pub struct PayloadBlock {
    pub data: Vec<u8>,
}

impl Debug for PayloadBlock {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PayloadBlock")
            .field("data (length)", &self.data.len())
            .finish()
    }
}

impl Serialize for PayloadBlock {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_bytes(&self.data)
    }
}

impl Validate for PayloadBlock {
    fn validate(&self) -> bool {
        return true;
    }
}
