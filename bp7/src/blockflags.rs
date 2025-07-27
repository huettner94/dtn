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

use bitflags::bitflags;
use serde::{Deserialize, Serialize, de::Visitor};

use crate::Validate;

bitflags! {
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
    pub struct BlockFlags: u64 {
        const MUST_REPLICATE_TO_ALL_FRAGMENTS = 0x01;
        const STATUS_REPORT_REQUESTED_WHEN_NOT_PROCESSABLE = 0x02;
        const DELETE_BUNDLE_WHEN_NOT_PROCESSABLE = 0x04;
        const DELETE_BLOCK_WHEN_NOT_PROCESSABLE = 0x10;
    }
}

impl Serialize for BlockFlags {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_u64(self.bits())
    }
}

impl<'de> Deserialize<'de> for BlockFlags {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct BlockFlagsVisitor;
        impl<'de> Visitor<'de> for BlockFlagsVisitor {
            type Value = BlockFlags;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("Block Flags")
            }

            fn visit_u64<E>(self, v: u64) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                Ok(BlockFlags::from_bits_truncate(v))
            }
        }
        deserializer.deserialize_u64(BlockFlagsVisitor)
    }
}

impl Validate for BlockFlags {
    fn validate(&self) -> bool {
        true
    }
}
