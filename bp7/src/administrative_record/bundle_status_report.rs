use serde::{
    de::{Error, Visitor},
    ser::SerializeSeq,
    Deserialize, Serialize,
};
use serde_repr::{Deserialize_repr, Serialize_repr};

use crate::{
    endpoint::Endpoint,
    time::{CreationTimestamp, DtnTime},
};

#[derive(Debug, PartialEq, Eq)]
pub struct BundleStatusItem {
    pub is_asserted: bool,
    pub timestamp: Option<DtnTime>,
}

impl Serialize for BundleStatusItem {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let length = if self.is_asserted && self.timestamp.is_some() {
            2
        } else {
            1
        };
        let mut seq = serializer.serialize_seq(Some(length))?;
        seq.serialize_element(&self.is_asserted)?;
        if length == 2 {
            seq.serialize_element(&self.timestamp)?;
        }
        seq.end()
    }
}

impl<'de> Deserialize<'de> for BundleStatusItem {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct BundleStatusItemVisitor;
        impl<'de> Visitor<'de> for BundleStatusItemVisitor {
            type Value = BundleStatusItem;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("bundle status item")
            }

            fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
            where
                A: serde::de::SeqAccess<'de>,
            {
                let length = seq.size_hint().ok_or(Error::custom(
                    "CBOR Array for BundleStatusItem must have a size hint",
                ))?;
                if length > 2 {
                    Err(Error::invalid_length(
                        length,
                        &"A BundleStatusItem must have 1 or 2 elements",
                    ))?;
                }
                let is_asserted = seq
                    .next_element()?
                    .ok_or(Error::custom("Error for field 'is_asserted'"))?;
                let timestamp = if length == 2 && is_asserted {
                    Some(seq.next_element()?).ok_or(Error::custom("Error for field 'timestamp'"))?
                } else {
                    None
                };
                Ok(BundleStatusItem {
                    is_asserted,
                    timestamp,
                })
            }
        }
        deserializer.deserialize_seq(BundleStatusItemVisitor)
    }
}

#[derive(Debug, PartialEq, Eq, Serialize_repr, Deserialize_repr)]
#[repr(u64)]
pub enum BundleStatusReason {
    NoAdditionalInformation = 0,
    LifetimeExpired = 1,
    ForwardedOverUnideirectionLink = 2,
    TransmissionCanceled = 3,
    DepletedStorage = 4,
    DestinationEndpointIDUnavailable = 5,
    NoKnownRouteToDestinationFromHere = 6,
    NoTimelyContactWithNextNodeOnRoute = 7,
    BlockUnintelligible = 8,
    HopLimitExceeded = 9,
    TrafficPared = 10,
    BlockUnsupported = 11,
}

#[derive(Debug, PartialEq, Eq)]
pub struct BundleStatusInformation {
    pub received_bundle: BundleStatusItem,
    pub forwarded_bundle: BundleStatusItem,
    pub delivered_bundle: BundleStatusItem,
    pub deleted_bundle: BundleStatusItem,
}

impl Serialize for BundleStatusInformation {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut seq = serializer.serialize_seq(Some(4))?;
        seq.serialize_element(&self.received_bundle)?;
        seq.serialize_element(&self.forwarded_bundle)?;
        seq.serialize_element(&self.delivered_bundle)?;
        seq.serialize_element(&self.deleted_bundle)?;
        seq.end()
    }
}

impl<'de> Deserialize<'de> for BundleStatusInformation {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct BundleStatusInformationVisitor;
        impl<'de> Visitor<'de> for BundleStatusInformationVisitor {
            type Value = BundleStatusInformation;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("bundle status information")
            }

            fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
            where
                A: serde::de::SeqAccess<'de>,
            {
                let length = seq.size_hint().ok_or(Error::custom(
                    "CBOR Array for BundleStatusInformation must have a size hint",
                ))?;
                if length != 4 {
                    Err(Error::invalid_length(
                        length,
                        &"A BundleStatusInformation must have 4 elements",
                    ))?;
                }
                let received_bundle = seq
                    .next_element()?
                    .ok_or(Error::custom("Error for field 'received_bundle'"))?;
                let forwarded_bundle = seq
                    .next_element()?
                    .ok_or(Error::custom("Error for field 'forwarded_bundle'"))?;
                let delivered_bundle = seq
                    .next_element()?
                    .ok_or(Error::custom("Error for field 'delivered_bundle'"))?;
                let deleted_bundle = seq
                    .next_element()?
                    .ok_or(Error::custom("Error for field 'deleted_bundle'"))?;
                Ok(BundleStatusInformation {
                    received_bundle,
                    forwarded_bundle,
                    delivered_bundle,
                    deleted_bundle,
                })
            }
        }
        deserializer.deserialize_seq(BundleStatusInformationVisitor)
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct BundleStatusReport {
    pub status_information: BundleStatusInformation,
    pub reason: BundleStatusReason,
    pub bundle_source: Endpoint,
    pub bundle_creation_timestamp: CreationTimestamp,
    pub fragment_offset: Option<u64>,
    pub fragment_length: Option<u64>,
}

impl Serialize for BundleStatusReport {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let length = if self.fragment_offset.is_some() && self.fragment_length.is_some() {
            6
        } else {
            4
        };
        let mut seq = serializer.serialize_seq(Some(length))?;

        seq.serialize_element(&self.status_information)?;
        seq.serialize_element(&self.reason)?;
        seq.serialize_element(&self.bundle_source)?;
        seq.serialize_element(&self.bundle_creation_timestamp)?;
        if length == 6 {
            seq.serialize_element(&self.fragment_offset.unwrap())?;
            seq.serialize_element(&self.fragment_length.unwrap())?;
        }
        seq.end()
    }
}

impl<'de> Deserialize<'de> for BundleStatusReport {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct BundleStatusReportVisitor;
        impl<'de> Visitor<'de> for BundleStatusReportVisitor {
            type Value = BundleStatusReport;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("bundle status report")
            }

            fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
            where
                A: serde::de::SeqAccess<'de>,
            {
                let length = seq.size_hint().ok_or(Error::custom(
                    "CBOR Array for BundleStatusReport must have a size hint",
                ))?;
                if length != 4 && length != 6 {
                    Err(Error::invalid_length(
                        length,
                        &"A BundleStatusReport must have 4 or 6 elements",
                    ))?;
                }
                let status_information = seq
                    .next_element()?
                    .ok_or(Error::custom("Error for field 'status_information'"))?;
                let reason = seq
                    .next_element()?
                    .ok_or(Error::custom("Error for field 'reason'"))?;
                let bundle_source = seq
                    .next_element()?
                    .ok_or(Error::custom("Error for field 'bundle_source'"))?;
                let bundle_creation_timestamp = seq
                    .next_element()?
                    .ok_or(Error::custom("Error for field 'bundle_creation_timestamp'"))?;
                let mut fragment_offset = None;
                let mut fragment_length = None;
                if length == 6 {
                    fragment_offset = Some(
                        seq.next_element()?
                            .ok_or(Error::custom("Error for field 'fragment_offset'"))?,
                    );
                    fragment_length = Some(
                        seq.next_element()?
                            .ok_or(Error::custom("Error for field 'fragment_length'"))?,
                    );
                }
                Ok(BundleStatusReport {
                    status_information,
                    reason,
                    bundle_source,
                    bundle_creation_timestamp,
                    fragment_offset,
                    fragment_length,
                })
            }
        }
        deserializer.deserialize_seq(BundleStatusReportVisitor)
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        administrative_record::bundle_status_report::{
            BundleStatusInformation, BundleStatusItem, BundleStatusReason, BundleStatusReport,
        },
        endpoint::Endpoint,
        time::{CreationTimestamp, DtnTime},
    };

    const BUNDLE_STATUS_REPORT_SERIALIZATION: &[u8] = &[
        0x84, 0x84, 0x82, 0xF5, 0x1A, 0x07, 0x5B, 0xCD, 0x15, 0x81, 0xF4, 0x81, 0xF4, 0x81, 0xF4,
        0x04, 0x82, 0x01, 0x6A, 0x2F, 0x2F, 0x74, 0x65, 0x73, 0x74, 0x2F, 0x61, 0x62, 0x63, 0x82,
        0x1A, 0x07, 0x5B, 0xCD, 0x15, 0x1A, 0x3A, 0xDE, 0x68, 0xB1,
    ];

    #[test]
    fn serialize_bundle_status_report() -> Result<(), serde_cbor::Error> {
        assert_eq!(
            serde_cbor::to_vec(&BundleStatusReport {
                status_information: BundleStatusInformation {
                    received_bundle: BundleStatusItem {
                        is_asserted: true,
                        timestamp: Some(DtnTime {
                            timestamp: 123456789
                        }),
                    },
                    forwarded_bundle: BundleStatusItem {
                        is_asserted: false,
                        timestamp: None
                    },
                    delivered_bundle: BundleStatusItem {
                        is_asserted: false,
                        timestamp: None
                    },
                    deleted_bundle: BundleStatusItem {
                        is_asserted: false,
                        timestamp: None
                    }
                },
                reason: BundleStatusReason::DepletedStorage,
                bundle_source: Endpoint::new("dtn://test/abc").unwrap(),
                bundle_creation_timestamp: CreationTimestamp {
                    creation_time: DtnTime {
                        timestamp: 123456789
                    },
                    sequence_number: 987654321,
                },
                fragment_offset: None,
                fragment_length: None
            })?,
            BUNDLE_STATUS_REPORT_SERIALIZATION
        );
        Ok(())
    }

    #[test]
    fn deserialize_creation_timestamp() -> Result<(), serde_cbor::Error> {
        let val: BundleStatusReport = serde_cbor::from_slice(BUNDLE_STATUS_REPORT_SERIALIZATION)?;
        assert_eq!(
            val,
            BundleStatusReport {
                status_information: BundleStatusInformation {
                    received_bundle: BundleStatusItem {
                        is_asserted: true,
                        timestamp: Some(DtnTime {
                            timestamp: 123456789
                        }),
                    },
                    forwarded_bundle: BundleStatusItem {
                        is_asserted: false,
                        timestamp: None
                    },
                    delivered_bundle: BundleStatusItem {
                        is_asserted: false,
                        timestamp: None
                    },
                    deleted_bundle: BundleStatusItem {
                        is_asserted: false,
                        timestamp: None
                    }
                },
                reason: BundleStatusReason::DepletedStorage,
                bundle_source: Endpoint::new("dtn://test/abc").unwrap(),
                bundle_creation_timestamp: CreationTimestamp {
                    creation_time: DtnTime {
                        timestamp: 123456789
                    },
                    sequence_number: 987654321,
                },
                fragment_offset: None,
                fragment_length: None
            }
        );
        Ok(())
    }
}
