use std::convert::TryFrom;

use serde::Serialize;

use crate::{endpoint::Endpoint, Validate};

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct PreviousNodeBlock {
    pub data: Vec<u8>,
}

impl Serialize for PreviousNodeBlock {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_bytes(&self.data)
    }
}

impl Validate for PreviousNodeBlock {
    fn validate(&self) -> bool {
        true
    }
}

impl TryFrom<PreviousNodeBlock> for Endpoint {
    type Error = serde_cbor::Error;

    fn try_from(value: PreviousNodeBlock) -> Result<Self, Self::Error> {
        serde_cbor::from_slice(&value.data)
    }
}

impl TryFrom<Endpoint> for PreviousNodeBlock {
    type Error = serde_cbor::Error;

    fn try_from(value: Endpoint) -> Result<Self, Self::Error> {
        let data = serde_cbor::to_vec(&value)?;
        Ok(PreviousNodeBlock { data })
    }
}
