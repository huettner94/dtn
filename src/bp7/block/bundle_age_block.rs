use std::convert::TryFrom;

use serde::{de::Visitor, Deserialize, Serialize};
use serde_cbor::Serializer;

use crate::bp7::Validate;

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct BundleAgeBlock {
    pub age: u64,
}

impl Serialize for BundleAgeBlock {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut vec = Vec::new();
        let inner_ser = &mut Serializer::new(&mut vec);
        serde::Serializer::serialize_u64(inner_ser, self.age)
            .or_else(|e| Err(serde::ser::Error::custom(e)))?;

        serializer.serialize_bytes(&vec)
    }
}

impl<'de> Deserialize<'de> for BundleAgeBlock {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct BundleAgeBlockVisitor;
        impl<'de> Visitor<'de> for BundleAgeBlockVisitor {
            type Value = BundleAgeBlock;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("Bundle Age Block")
            }

            fn visit_u64<E>(self, v: u64) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                Ok(BundleAgeBlock { age: v })
            }
        }
        deserializer.deserialize_u64(BundleAgeBlockVisitor)
    }
}

impl Validate for BundleAgeBlock {
    fn validate(&self) -> bool {
        return true;
    }
}

impl TryFrom<Vec<u8>> for BundleAgeBlock {
    type Error = serde_cbor::Error;

    fn try_from(value: Vec<u8>) -> Result<Self, Self::Error> {
        return serde_cbor::from_slice(&value);
    }
}
