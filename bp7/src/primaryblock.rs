use serde::{de::Error, de::Visitor, ser::SerializeSeq, Deserialize, Serialize};

use crate::{
    bundleflags::BundleFlags, crc::CRCType, endpoint::Endpoint, time::CreationTimestamp, *,
};

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct PrimaryBlock {
    pub version: u64,
    pub bundle_processing_flags: BundleFlags,
    pub crc: CRCType,
    pub destination_endpoint: Endpoint,
    pub source_node: Endpoint,
    pub report_to: Endpoint,
    pub creation_timestamp: CreationTimestamp,
    pub lifetime: u64,
    pub fragment_offset: Option<u64>,
    pub total_data_length: Option<u64>,
}

impl Serialize for PrimaryBlock {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let len = if self.crc == CRCType::NoCRC && self.fragment_offset.is_none() {
            8
        } else if self.crc != CRCType::NoCRC && self.fragment_offset.is_none() {
            9
        } else if self.crc == CRCType::NoCRC && self.fragment_offset.is_some() {
            10
        } else {
            11
        };
        let mut seq = serializer.serialize_seq(Some(len))?;
        seq.serialize_element(&self.version)?;
        seq.serialize_element(&self.bundle_processing_flags)?;
        seq.serialize_element(&self.crc)?;
        seq.serialize_element(&self.destination_endpoint)?;
        seq.serialize_element(&self.source_node)?;
        seq.serialize_element(&self.report_to)?;
        seq.serialize_element(&self.creation_timestamp)?;
        seq.serialize_element(&self.lifetime)?;
        if self.fragment_offset.is_some() {
            seq.serialize_element(&self.fragment_offset.unwrap())?;
            seq.serialize_element(&self.total_data_length.unwrap())?;
        }
        if self.crc != CRCType::NoCRC {
            match self.crc {
                CRCType::NoCRC => panic!("Attempting to serialize content when we dont have a CRC"),
                CRCType::CRC16(x) => seq.serialize_element(&x)?,
                CRCType::CRC32(x) => seq.serialize_element(&x)?,
            };
        }
        seq.end()
    }
}

impl<'de> Deserialize<'de> for PrimaryBlock {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct PrimaryBlockVisitor;
        impl<'de> Visitor<'de> for PrimaryBlockVisitor {
            type Value = PrimaryBlock;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("primary block")
            }

            fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
            where
                A: serde::de::SeqAccess<'de>,
            {
                let size = seq.size_hint().unwrap();
                if !(8..=11).contains(&size) {
                    return Err(Error::invalid_length(
                        size,
                        &"Primary block has 8 to 11 elements",
                    ));
                }
                let version = seq
                    .next_element()?
                    .ok_or(Error::custom("Error for field 'version'"))?;
                let bundle_processing_flags = seq
                    .next_element()?
                    .ok_or(Error::custom("Error for field 'bundle_processing_flags'"))?;
                let mut crc: CRCType = seq
                    .next_element()?
                    .ok_or(Error::custom("Error for field 'crc_type'"))?;
                let destination_endpoint = seq
                    .next_element()?
                    .ok_or(Error::custom("Error for field 'destination_endpoint'"))?;
                let source_node = seq
                    .next_element()?
                    .ok_or(Error::custom("Error for field 'source_node'"))?;
                let report_to = seq
                    .next_element()?
                    .ok_or(Error::custom("Error for field 'report_to'"))?;
                let creation_timestamp = seq
                    .next_element()?
                    .ok_or(Error::custom("Error for field 'creation_timestamp'"))?;
                let lifetime = seq
                    .next_element()?
                    .ok_or(Error::custom("Error for field 'lifetime'"))?;

                let (fragment_offset, total_data_length) = if size == 10 || size == 11 {
                    (
                        Some(
                            seq.next_element()?
                                .ok_or(Error::custom("Error for field 'fragment_offset'"))?,
                        ),
                        Some(
                            seq.next_element()?
                                .ok_or(Error::custom("Error for field 'total_data_length'"))?,
                        ),
                    )
                } else {
                    (None, None)
                };

                if size == 9 || size == 11 {
                    crc = crc.deserialize_value(seq)?;
                }

                Ok(PrimaryBlock {
                    version,
                    bundle_processing_flags,
                    crc,
                    destination_endpoint,
                    source_node,
                    report_to,
                    creation_timestamp,
                    lifetime,
                    fragment_offset,
                    total_data_length,
                })
            }
        }
        deserializer.deserialize_seq(PrimaryBlockVisitor)
    }
}

impl Validate for PrimaryBlock {
    fn validate(&self) -> bool {
        if self.version != 7 {
            return false;
        }
        if self.fragment_offset.is_some() != self.total_data_length.is_some() {
            return false;
        }
        if !self.source_node.validate() {
            return false;
        }
        if !self.destination_endpoint.validate() {
            return false;
        }
        if !self.report_to.validate() {
            return false;
        }
        true
    }
}

impl PrimaryBlock {
    pub fn equals_ignoring_fragment_offset(&self, other: &PrimaryBlock) -> bool {
        let self_cleaned = PrimaryBlock {
            fragment_offset: None,
            ..self.clone()
        };
        let other_cleaned = PrimaryBlock {
            fragment_offset: None,
            ..other.clone()
        };
        self_cleaned == other_cleaned
    }

    pub fn equals_ignoring_fragment_info(&self, other: &PrimaryBlock) -> bool {
        let self_cleaned = PrimaryBlock {
            fragment_offset: None,
            total_data_length: None,
            ..self.clone()
        };
        let other_cleaned = PrimaryBlock {
            fragment_offset: None,
            total_data_length: None,
            ..other.clone()
        };
        self_cleaned == other_cleaned
    }
}
