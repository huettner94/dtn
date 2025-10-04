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

use serde::Serialize;

use crate::{Validate, endpoint::Endpoint};

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct PreviousNodeBlock {
    //TODO: this should probably be something more reasonable
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
