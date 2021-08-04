use serde::Serialize;

use crate::Validate;

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct PayloadBlock {
    pub data: Vec<u8>,
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