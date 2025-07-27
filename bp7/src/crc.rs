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

use std::convert::TryInto;

use serde::{
    Deserialize, Serialize,
    de::{Error, Unexpected, Visitor},
};

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
#[repr(u64)]
pub enum CRCType {
    NoCRC,
    CRC16([u8; 2]),
    CRC32([u8; 4]),
}

impl Serialize for CRCType {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_u64(match self {
            CRCType::NoCRC => 0,
            CRCType::CRC16(_) => 1,
            CRCType::CRC32(_) => 2,
        })
    }
}

impl<'de> Deserialize<'de> for CRCType {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct CRCVisitor;
        impl<'de> Visitor<'de> for CRCVisitor {
            type Value = CRCType;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("crc type")
            }

            fn visit_u64<E>(self, v: u64) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                match v {
                    0 => Ok(CRCType::NoCRC),
                    1 => Ok(CRCType::CRC16([0; 2])),
                    2 => Ok(CRCType::CRC32([0; 4])),
                    _ => Err(serde::de::Error::invalid_value(
                        Unexpected::Unsigned(v),
                        &"crc type must be bbetween 0 to 2",
                    )),
                }
            }
        }
        deserializer.deserialize_u64(CRCVisitor)
    }
}

impl CRCType {
    pub fn deserialize_value<'de, A>(&self, mut seq: A) -> Result<CRCType, A::Error>
    where
        A: serde::de::SeqAccess<'de>,
    {
        match self {
            CRCType::NoCRC => {
                panic!("Attempting to deserialize content when we dont have a CRC")
            }
            CRCType::CRC16(_) => {
                let val: &[u8] = seq
                    .next_element()?
                    .ok_or(Error::custom("Error for crc content"))?;
                let len = val.len();
                let arr: [u8; 2] = match val.try_into() {
                    Ok(v) => v,
                    Err(_) => {
                        return Err(Error::invalid_length(len, &"Expected 2 bytes for crc16"));
                    }
                };
                Ok(CRCType::CRC16(arr))
            }
            CRCType::CRC32(_) => {
                let val: &[u8] = seq
                    .next_element()?
                    .ok_or(Error::custom("Error for crc content"))?;
                let len = val.len();
                let arr: [u8; 4] = match val.try_into() {
                    Ok(v) => v,
                    Err(_) => {
                        return Err(Error::invalid_length(len, &"Expected 4 bytes for crc32"));
                    }
                };
                Ok(CRCType::CRC32(arr))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::crc::CRCType;

    #[test]
    fn serialize_nocrc() -> Result<(), serde_cbor::Error> {
        assert_eq!(serde_cbor::to_vec(&CRCType::NoCRC)?, [0]);
        Ok(())
    }

    #[test]
    fn serialize_crc16() -> Result<(), serde_cbor::Error> {
        assert_eq!(serde_cbor::to_vec(&CRCType::CRC16([0x55, 0xAA]))?, [1]);
        Ok(())
    }

    #[test]
    fn serialize_crc32() -> Result<(), serde_cbor::Error> {
        assert_eq!(
            serde_cbor::to_vec(&CRCType::CRC32([0x55, 0xAA, 0x55, 0xAA]))?,
            [2]
        );
        Ok(())
    }

    #[test]
    fn deserialize_nocrc() -> Result<(), serde_cbor::Error> {
        let val: CRCType = serde_cbor::from_slice(&[0])?;
        assert_eq!(val, CRCType::NoCRC);
        Ok(())
    }

    #[test]
    fn deserialize_crc16() -> Result<(), serde_cbor::Error> {
        let val: CRCType = serde_cbor::from_slice(&[1])?;
        assert_eq!(val, CRCType::CRC16([0; 2]));
        Ok(())
    }

    #[test]
    fn deserialize_crc32() -> Result<(), serde_cbor::Error> {
        let val: CRCType = serde_cbor::from_slice(&[2])?;
        assert_eq!(val, CRCType::CRC32([0; 4]));
        Ok(())
    }
}
