use bitflags::bitflags;
use serde::{Deserialize, Serialize};

use crate::Validate;

bitflags! {
    #[derive(Serialize, Deserialize)]
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
