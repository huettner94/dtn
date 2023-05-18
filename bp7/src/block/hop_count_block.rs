use std::convert::TryFrom;

use serde::{
    de::{Error, Visitor},
    ser::SerializeSeq,
    Deserialize, Serialize,
};
use serde_cbor::Serializer;

use crate::Validate;

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct HopCountBlock {
    pub limit: u64,
    pub count: u64,
}

impl Serialize for HopCountBlock {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut vec = Vec::new();
        let inner_ser = &mut Serializer::new(&mut vec);
        let mut seq = serde::Serializer::serialize_seq(inner_ser, Some(2)).map_err(serde::ser::Error::custom)?;
        seq.serialize_element(&self.limit).map_err(serde::ser::Error::custom)?;
        seq.serialize_element(&self.count).map_err(serde::ser::Error::custom)?;
        seq.end().map_err(serde::ser::Error::custom)?;

        serializer.serialize_bytes(&vec)
    }
}

impl<'de> Deserialize<'de> for HopCountBlock {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct HopCountBlockVisitor;
        impl<'de> Visitor<'de> for HopCountBlockVisitor {
            type Value = HopCountBlock;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("Hop Count Block")
            }

            fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
            where
                A: serde::de::SeqAccess<'de>,
            {
                let size = seq.size_hint().ok_or_else(|| {
                    Error::custom("Hop Count Block must know the length of its contents")
                })?;
                if size != 2 {
                    return Err(Error::invalid_length(
                        size,
                        &"Hop Count Block has 2 elements",
                    ));
                }

                let limit: u64 = seq
                    .next_element()?
                    .ok_or(Error::custom("Error for field 'limit'"))?;

                let count: u64 = seq
                    .next_element()?
                    .ok_or(Error::custom("Error for field 'count'"))?;
                Ok(HopCountBlock { limit, count })
            }
        }
        deserializer.deserialize_seq(HopCountBlockVisitor)
    }
}

impl Validate for HopCountBlock {
    fn validate(&self) -> bool {
        self.limit <= 255
    }
}

impl TryFrom<Vec<u8>> for HopCountBlock {
    type Error = serde_cbor::Error;

    fn try_from(value: Vec<u8>) -> Result<Self, Self::Error> {
        serde_cbor::from_slice(&value)
    }
}
