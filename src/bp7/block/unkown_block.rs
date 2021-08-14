use serde::Serialize;

use crate::bp7::Validate;

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct UnkownBlock {
    pub block_type: u64,
    pub data: Vec<u8>,
}

impl Serialize for UnkownBlock {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_bytes(&self.data)
    }
}

impl Validate for UnkownBlock {
    fn validate(&self) -> bool {
        return true;
    }
}
