use serde::{
    de::{Error, Visitor},
    ser::SerializeSeq,
    Deserialize, Serialize,
};
use serde_repr::{Deserialize_repr, Serialize_repr};

use crate::administrative_record::bundle_status_report::BundleStatusReport;

pub mod bundle_status_report;

#[derive(Debug, Serialize_repr, Deserialize_repr, PartialEq, Eq)]
#[repr(u64)]
enum AdministrativeRecordType {
    BundleStatusReport = 1,
}

pub enum AdministrativeRecord {
    BundleStatusReport(BundleStatusReport),
}

impl Serialize for AdministrativeRecord {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut seq = serializer.serialize_seq(Some(2))?;
        match self {
            AdministrativeRecord::BundleStatusReport(e) => {
                seq.serialize_element(&AdministrativeRecordType::BundleStatusReport)?;
                seq.serialize_element(e)?;
            }
        }
        seq.end()
    }
}

impl<'de> Deserialize<'de> for AdministrativeRecord {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct AdministrativeRecordVisitor;
        impl<'de> Visitor<'de> for AdministrativeRecordVisitor {
            type Value = AdministrativeRecord;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("administrative record")
            }

            fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
            where
                A: serde::de::SeqAccess<'de>,
            {
                let administrative_record_type: AdministrativeRecordType =
                    seq.next_element()?.ok_or(Error::custom(
                        "Error for field 'administrative_record_type'",
                    ))?;
                match administrative_record_type {
                    AdministrativeRecordType::BundleStatusReport => {
                        let bundle_status_report: BundleStatusReport = seq
                            .next_element()?
                            .ok_or(Error::custom("Error for field 'bundle_status_report'"))?;
                        return Ok(AdministrativeRecord::BundleStatusReport(
                            bundle_status_report,
                        ));
                    }
                }
            }
        }
        deserializer.deserialize_seq(AdministrativeRecordVisitor)
    }
}
