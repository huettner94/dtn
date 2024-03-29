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

use serde::{
    de::{Error, Visitor},
    ser::SerializeSeq,
    Deserialize, Serialize,
};
use serde_repr::{Deserialize_repr, Serialize_repr};

use crate::{administrative_record::bundle_status_report::BundleStatusReport, SerializationError};

pub mod bundle_status_report;

#[derive(Debug, Serialize_repr, Deserialize_repr, PartialEq, Eq)]
#[repr(u64)]
enum AdministrativeRecordType {
    BundleStatusReport = 1,
}

#[derive(Debug)]
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
                        Ok(AdministrativeRecord::BundleStatusReport(
                            bundle_status_report,
                        ))
                    }
                }
            }
        }
        deserializer.deserialize_seq(AdministrativeRecordVisitor)
    }
}

impl TryFrom<Vec<u8>> for AdministrativeRecord {
    type Error = SerializationError;

    fn try_from(value: Vec<u8>) -> Result<Self, Self::Error> {
        serde_cbor::from_slice(&value).map_err(SerializationError::SerializationError)
    }
}

impl TryFrom<&Vec<u8>> for AdministrativeRecord {
    type Error = SerializationError;

    fn try_from(value: &Vec<u8>) -> Result<Self, Self::Error> {
        serde_cbor::from_slice(value).map_err(SerializationError::SerializationError)
    }
}

impl TryFrom<AdministrativeRecord> for Vec<u8> {
    type Error = SerializationError;

    fn try_from(value: AdministrativeRecord) -> Result<Self, Self::Error> {
        (&value).try_into()
    }
}

impl TryFrom<&AdministrativeRecord> for Vec<u8> {
    type Error = SerializationError;

    fn try_from(value: &AdministrativeRecord) -> Result<Self, Self::Error> {
        serde_cbor::to_vec(value).map_err(SerializationError::SerializationError)
    }
}
