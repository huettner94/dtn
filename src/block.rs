use serde::{de::Error, de::Visitor, ser::SerializeSeq, Deserialize, Serialize};

use crate::{blockflags::BlockFlags, crc::CRCType, *};

use serde_repr::{Deserialize_repr, Serialize_repr};

#[derive(Debug, Serialize_repr, Deserialize_repr, PartialEq, Eq)]
#[repr(u64)]
pub enum BlockType {
    PAYLOAD = 1,
    //TODO add more
}

#[derive(Debug)]
pub struct Block {
    pub block_type: BlockType,
    pub block_number: u64,
    pub block_flags: BlockFlags,
    pub crc: CRCType,
    pub data: Vec<u8>,
}

impl Serialize for Block {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let len = if self.crc == CRCType::NoCRC { 5 } else { 6 };
        let mut seq = serializer.serialize_seq(Some(len))?;
        seq.serialize_element(&self.block_type)?;
        seq.serialize_element(&self.block_number)?;
        seq.serialize_element(&self.block_flags)?;
        seq.serialize_element(&self.crc)?;
        seq.serialize_element(&self.data)?;
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

impl<'de> Deserialize<'de> for Block {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct BlockVisitor;
        impl<'de> Visitor<'de> for BlockVisitor {
            type Value = Block;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("block")
            }

            fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
            where
                A: serde::de::SeqAccess<'de>,
            {
                let size = seq.size_hint().unwrap();
                if size < 5 || size > 6 {
                    return Err(Error::invalid_length(size, &"Block has 5 to 6 elements"));
                }
                let block_type = seq
                    .next_element()?
                    .ok_or(Error::custom("Error for field 'block_type'"))?;
                let block_number = seq
                    .next_element()?
                    .ok_or(Error::custom("Error for field 'block_number'"))?;
                let block_flags = seq
                    .next_element()?
                    .ok_or(Error::custom("Error for field 'block_flags'"))?;
                let mut crc: CRCType = seq
                    .next_element()?
                    .ok_or(Error::custom("Error for field 'crc_type'"))?;
                let data: &[u8] = seq
                    .next_element()?
                    .ok_or(Error::custom("Error for field 'data'"))?;

                if size == 6 {
                    crc = crc.deserialize_value(seq)?;
                }

                return Ok(Block {
                    block_type,
                    block_number,
                    block_flags,
                    crc,
                    data: Vec::from(data),
                });
            }
        }
        deserializer.deserialize_seq(BlockVisitor)
    }
}

impl Validate for Block {
    fn validate(&self) -> bool {
        return true;
    }
}
