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

use serde::{Deserialize, Serialize};
use serde_cbor::Serializer;

use crate::{Validate, endpoint::Endpoint};

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct PreviousNodeBlock {
    pub previous_node: Endpoint,
}

impl Serialize for PreviousNodeBlock {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut vec = Vec::new();
        let inner_ser = &mut Serializer::new(&mut vec);
        self.previous_node
            .serialize(inner_ser)
            .map_err(serde::ser::Error::custom)?;

        serializer.serialize_bytes(&vec)
    }
}

impl<'de> Deserialize<'de> for PreviousNodeBlock {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let endpoint = Endpoint::deserialize(deserializer)?;
        Ok(PreviousNodeBlock {
            previous_node: endpoint,
        })
    }
}

impl Validate for PreviousNodeBlock {
    fn validate(&self) -> bool {
        true
    }
}

impl TryFrom<Vec<u8>> for PreviousNodeBlock {
    type Error = serde_cbor::Error;

    fn try_from(value: Vec<u8>) -> Result<Self, Self::Error> {
        serde_cbor::from_slice(&value)
    }
}
