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
    pub struct BundleFlags: u64 {
        const FRAGMENT = 0x000001;
        const ADMINISTRATIVE_RECORD = 0x000002;
        const MUST_NOT_FRAGMENT = 0x000004;
        const APPLICATION_ACKNOWLEGEMENT_REQUESTED = 0x000020;
        const STATUS_TIME_REQUESTED = 0x000040;
        const BUNDLE_RECEIPTION_STATUS_REQUESTED = 0x004000;
        const BUNDLE_FORWARDING_STATUS_REQUEST = 0x010000;
        const BUNDLE_DELIVERY_STATUS_REQUESTED = 0x020000;
        const BUNDLE_DELETION_STATUS_REQUESTED = 0x040000;
    }
}

impl Serialize for BundleFlags {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_u64(self.bits())
    }
}

impl<'de> Deserialize<'de> for BundleFlags {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct BundleFlagsVisitor;
        impl<'de> Visitor<'de> for BundleFlagsVisitor {
            type Value = BundleFlags;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("Bundle Flags")
            }

            fn visit_u64<E>(self, v: u64) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                Ok(BundleFlags::from_bits_truncate(v))
            }
        }
        deserializer.deserialize_u64(BundleFlagsVisitor)
    }
}

impl Validate for BundleFlags {
    fn validate(&self) -> bool {
        if self.contains(BundleFlags::ADMINISTRATIVE_RECORD)
            && self.intersects(
                BundleFlags::BUNDLE_RECEIPTION_STATUS_REQUESTED
                    | BundleFlags::BUNDLE_FORWARDING_STATUS_REQUEST
                    | BundleFlags::BUNDLE_DELIVERY_STATUS_REQUESTED
                    | BundleFlags::BUNDLE_DELETION_STATUS_REQUESTED,
            )
        {
            return false;
        }
        true
    }
}
