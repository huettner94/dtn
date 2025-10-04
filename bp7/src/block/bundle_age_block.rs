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

use std::convert::TryFrom;

use serde::{Deserialize, Serialize, de::Visitor};
use serde_cbor::Serializer;

use crate::Validate;

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
        serde::Serializer::serialize_u64(inner_ser, self.age).map_err(serde::ser::Error::custom)?;

        serializer.serialize_bytes(&vec)
    }
}

impl<'de> Deserialize<'de> for BundleAgeBlock {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct BundleAgeBlockVisitor;
        impl Visitor<'_> for BundleAgeBlockVisitor {
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
        true
    }
}

impl TryFrom<Vec<u8>> for BundleAgeBlock {
    type Error = serde_cbor::Error;

    fn try_from(value: Vec<u8>) -> Result<Self, Self::Error> {
        serde_cbor::from_slice(&value)
    }
}
