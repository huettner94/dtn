use serde::{Deserialize, Serialize};
use serde_repr::{Deserialize_repr, Serialize_repr};

pub mod block;
pub mod bundle;
pub mod crc;

pub trait Validate {
    fn validate(&self) -> bool;
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CreationTimestamp {
    pub creation_time: DtnTime,
    pub sequence_number: u64,
}

type DtnTime = u64;

#[derive(Debug, Serialize_repr, Deserialize_repr, PartialEq, Eq)]
#[repr(u64)]
pub enum EndpointType {
    DTN = 1,
    IPN = 2,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct EndpointID {
    pub endpoint_type: EndpointType,
    pub endpoint: String, // TODO: invalid for IPN
}

#[derive(Debug, PartialEq, Eq)]
#[repr(u64)]
pub enum BlockFlags {
    MustReplicateToAllFragments = 0x01,
    StatusReportRequestedWhenNotProcessable = 0x02,
    DeleteBundleWhenNotProcessable = 0x04,
    DeleteBlockWhenNotProcessable = 0x10,
}

#[derive(Debug, PartialEq, Eq)]
#[repr(u64)]
pub enum BundleFlags {
    Fragment = 0x000001,
    AdministrativeRecord = 0x000002,
    MustNotFragment = 0x000004,
    ApplicationAcknowlegementRequested = 0x000020,
    StatusTimeRequested = 0x000040,
    BundleReceiptionStatusRequested = 0x004000,
    BundleForwardingStatusRequest = 0x010000,
    BundleDeliveryStatusRequested = 0x020000,
    BundleDeletionStatusRequested = 0x040000,
}
