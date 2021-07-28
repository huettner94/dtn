use serde::{
    de::{Unexpected, Visitor},
    Deserialize, Serialize,
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
        return serializer.serialize_u64(match self {
            CRCType::NoCRC => 0,
            CRCType::CRC16(_) => 1,
            CRCType::CRC32(_) => 2,
        });
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
                        &"crc type must be 0 to 2",
                    )),
                }
            }
        }
        deserializer.deserialize_u64(CRCVisitor)
    }
}
