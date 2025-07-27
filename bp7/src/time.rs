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

use std::fmt::Debug;

use chrono::{DateTime, TimeZone, Utc};
use serde::{
    Deserialize, Serialize,
    de::{Error, Visitor},
    ser::SerializeSeq,
};

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct CreationTimestamp {
    pub creation_time: DtnTime,
    pub sequence_number: u64,
}

impl Serialize for CreationTimestamp {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut seq = serializer.serialize_seq(Some(2))?;
        seq.serialize_element(&self.creation_time)?;
        seq.serialize_element(&self.sequence_number)?;
        seq.end()
    }
}

impl<'de> Deserialize<'de> for CreationTimestamp {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct CreationTimestampVisitor;
        impl<'de> Visitor<'de> for CreationTimestampVisitor {
            type Value = CreationTimestamp;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("creation time")
            }

            fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
            where
                A: serde::de::SeqAccess<'de>,
            {
                let creation_time = seq
                    .next_element()?
                    .ok_or(Error::custom("Error for field 'creation_time'"))?;
                let sequence_number = seq
                    .next_element()?
                    .ok_or(Error::custom("Error for field 'sequence_number'"))?;
                Ok(CreationTimestamp {
                    creation_time,
                    sequence_number,
                })
            }
        }
        deserializer.deserialize_seq(CreationTimestampVisitor)
    }
}

#[derive(PartialEq, Eq, PartialOrd, Ord, Clone, Copy, Serialize, Deserialize)]
#[serde(transparent)]
pub struct DtnTime {
    pub timestamp: u64,
}

impl Debug for DtnTime {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let datetime: DateTime<Utc> = self.into();
        f.write_fmt(format_args!(
            "DtnTime {{ {}; timestamp: {} }}",
            datetime, self.timestamp
        ))
    }
}

impl From<DtnTime> for DateTime<Utc> {
    fn from(dtn: DtnTime) -> Self {
        DateTime::from(&dtn)
    }
}

impl From<&DtnTime> for DateTime<Utc> {
    fn from(dtn: &DtnTime) -> Self {
        let millis = (dtn.timestamp as i64) + 946684800000; // 946684800 seconds between 1970-01-01 and 2000-01-01
        Utc.timestamp_millis_opt(millis).unwrap()
    }
}

impl From<DateTime<Utc>> for DtnTime {
    fn from(utc: DateTime<Utc>) -> Self {
        DtnTime::from(&utc)
    }
}

impl From<&DateTime<Utc>> for DtnTime {
    fn from(utc: &DateTime<Utc>) -> Self {
        let millis = utc.timestamp_millis() - 946684800000; // 946684800 seconds between 1970-01-01 and 2000-01-01
        DtnTime {
            timestamp: millis as u64,
        }
    }
}

impl DtnTime {
    pub fn now() -> Self {
        Utc::now().into()
    }
}

#[cfg(test)]
mod tests {
    use crate::time::{CreationTimestamp, DtnTime};

    const CREATION_TIMESTAMP_SERIALIZATION: &[u8] = &[
        0x82, 0x1A, 0x07, 0x5B, 0xCD, 0x15, 0x1A, 0x3A, 0xDE, 0x68, 0xB1,
    ];

    #[test]
    fn serialize_creation_timestamp() -> Result<(), serde_cbor::Error> {
        assert_eq!(
            serde_cbor::to_vec(&CreationTimestamp {
                creation_time: DtnTime {
                    timestamp: 123456789
                },
                sequence_number: 987654321
            })?,
            CREATION_TIMESTAMP_SERIALIZATION
        );
        Ok(())
    }

    #[test]
    fn deserialize_creation_timestamp() -> Result<(), serde_cbor::Error> {
        let val: CreationTimestamp = serde_cbor::from_slice(CREATION_TIMESTAMP_SERIALIZATION)?;
        assert_eq!(
            val,
            CreationTimestamp {
                creation_time: DtnTime {
                    timestamp: 123456789
                },
                sequence_number: 987654321
            }
        );
        Ok(())
    }

    const DTNTIME_SERIALIZATION: &[u8] = &[0x1A, 0x07, 0x5B, 0xCD, 0x15];

    #[test]
    fn serialize_dtntime() -> Result<(), serde_cbor::Error> {
        assert_eq!(
            serde_cbor::to_vec(&DtnTime {
                timestamp: 123456789
            })?,
            DTNTIME_SERIALIZATION
        );
        Ok(())
    }

    #[test]
    fn deserialize_dtntime() -> Result<(), serde_cbor::Error> {
        let val: DtnTime = serde_cbor::from_slice(DTNTIME_SERIALIZATION)?;
        assert_eq!(
            val,
            DtnTime {
                timestamp: 123456789
            }
        );
        Ok(())
    }
}
