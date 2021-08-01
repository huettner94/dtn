use std::fmt::Debug;

use chrono::{DateTime, TimeZone, Utc};
use serde::{de::Visitor, Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct CreationTimestamp {
    pub creation_time: DtnTime,
    pub sequence_number: u64,
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
        let millis = (dtn.timestamp as i64) + 946684800000; // 946684800 seconds between 1970-01-01 and 2000-01-01
        return Utc.timestamp_millis(millis);
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
        let millis = utc.timestamp_millis() - 946684800000; // 946684800 seconds between 1970-01-01 and 2000-01-01
        return DtnTime {
            timestamp: millis as u64,
        };
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
