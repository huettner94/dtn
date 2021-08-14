use bitflags::bitflags;
use serde::{de::Visitor, Deserialize, Serialize};

use crate::bp7::Validate;

bitflags! {
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
        return true;
    }
}
