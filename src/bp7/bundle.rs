use std::{
    convert::{TryFrom, TryInto},
    fmt::Write,
};

use binascii::hex2bin;
use serde::{de::Error, de::Visitor, ser::SerializeSeq, Deserialize, Serialize};

use crate::bp7::{block::CanonicalBlock, primaryblock::PrimaryBlock, SerializationError, Validate};

#[derive(Debug, PartialEq, Eq)]
pub struct Bundle {
    pub primary_block: PrimaryBlock,
    pub blocks: Vec<CanonicalBlock>,
}

impl Serialize for Bundle {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut seq = serializer.serialize_seq(None)?;
        seq.serialize_element(&self.primary_block)?;
        for block in &self.blocks {
            seq.serialize_element(&block)?;
        }
        seq.end()
    }
}

impl<'de> Deserialize<'de> for Bundle {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct BundleVisitor;
        impl<'de> Visitor<'de> for BundleVisitor {
            type Value = Bundle;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("bundle")
            }

            fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
            where
                A: serde::de::SeqAccess<'de>,
            {
                let mut blocks: Vec<CanonicalBlock> = match seq.size_hint() {
                    Some(v) => Vec::with_capacity(v),
                    None => Vec::new(),
                };
                let primary_block = seq
                    .next_element()?
                    .ok_or(Error::custom("Error for field 'primary_block'"))?;
                while let Some(block) = seq.next_element()? {
                    blocks.push(block);
                }

                if blocks.len() < 1 {
                    return Err(Error::invalid_length(0, &"must have at least one block"));
                }

                return Ok(Bundle {
                    primary_block,
                    blocks,
                });
            }
        }
        deserializer.deserialize_seq(BundleVisitor)
    }
}

impl Validate for Bundle {
    fn validate(&self) -> bool {
        if !self.primary_block.validate() {
            return false;
        }
        for block in &self.blocks {
            if !block.validate() {
                return false;
            }
        }
        return true;
    }
}

impl TryFrom<Vec<u8>> for Bundle {
    type Error = SerializationError;

    fn try_from(value: Vec<u8>) -> Result<Self, Self::Error> {
        serde_cbor::from_slice(&value).or_else(|e| Err(SerializationError::SerializationError(e)))
    }
}

impl TryFrom<Bundle> for Vec<u8> {
    type Error = SerializationError;

    fn try_from(value: Bundle) -> Result<Self, Self::Error> {
        (&value).try_into()
    }
}

impl TryFrom<&Bundle> for Vec<u8> {
    type Error = SerializationError;

    fn try_from(value: &Bundle) -> Result<Self, Self::Error> {
        serde_cbor::to_vec(value).or_else(|e| Err(SerializationError::SerializationError(e)))
    }
}

impl Bundle {
    pub fn as_hex(&self) -> Result<String, SerializationError> {
        let vec: Vec<u8> = self.try_into()?;
        let mut s = String::with_capacity(2 * vec.len());
        for b in vec {
            write!(&mut s, "{:02X?}", &b).or_else(|_| Err(SerializationError::ConversionError))?;
        }
        return Ok(s);
    }

    pub fn from_hex(hex: &str) -> Result<Bundle, SerializationError> {
        let mut val = vec![0; hex.len() / 2];
        hex2bin(hex.as_bytes(), &mut val).unwrap();
        val.try_into()
    }
}
