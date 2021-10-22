use std::fmt::Debug;

use chrono::{DateTime, TimeZone, Utc};
use serde::{
    de::{Error, Visitor},
    ser::SerializeSeq,
    Deserialize, Serialize,
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
                return Ok(CreationTimestamp {
                    creation_time,
                    sequence_number,
                });
            }
        }
        deserializer.deserialize_seq(CreationTimestampVisitor)
    }
}

#[derive(PartialEq, Eq, PartialOrd, Ord, Clone, Copy)]
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

impl Serialize for DtnTime {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_u64(self.timestamp)
    }
}

impl<'de> Deserialize<'de> for DtnTime {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct DtnTimeVisitor;
        impl<'de> Visitor<'de> for DtnTimeVisitor {
            type Value = DtnTime;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("dtn time")
            }

            fn visit_u64<E>(self, v: u64) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                return Ok(DtnTime { timestamp: v });
            }
        }
        deserializer.deserialize_u64(DtnTimeVisitor)
    }
}

impl From<DtnTime> for DateTime<Utc> {
    fn from(dtn: DtnTime) -> Self {
        return DateTime::from(&dtn);
    }
}

impl From<&DtnTime> for DateTime<Utc> {
    fn from(dtn: &DtnTime) -> Self {
        let millis = (dtn.timestamp as i64) + 946684800000; // 946684800 seconds between 1970-01-01 and 2000-01-01
        return Utc.timestamp_millis(millis);
    }
}

impl From<DateTime<Utc>> for DtnTime {
    fn from(utc: DateTime<Utc>) -> Self {
        return DtnTime::from(&utc);
    }
}

impl From<&DateTime<Utc>> for DtnTime {
    fn from(utc: &DateTime<Utc>) -> Self {
        let millis = utc.timestamp_millis() - 946684800000; // 946684800 seconds between 1970-01-01 and 2000-01-01
        return DtnTime {
            timestamp: millis as u64,
        };
    }
}

impl DtnTime {
    pub fn now() -> Self {
        Utc::now().into()
    }
}
