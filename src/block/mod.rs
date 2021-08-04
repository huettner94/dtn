use std::convert::TryFrom;

use serde::{de::Error, de::Visitor, ser::SerializeSeq, Deserialize, Serialize};

use crate::{blockflags::BlockFlags, crc::CRCType, *};

use self::{payload_block::PayloadBlock, unkown_block::UnkownBlock};
use num_enum::IntoPrimitive;
use num_enum::TryFromPrimitive;
use serde_repr::{Deserialize_repr, Serialize_repr};

mod payload_block;
mod unkown_block;

#[derive(
    Debug,
    Serialize_repr,
    Deserialize_repr,
    PartialEq,
    Eq,
    Clone,
    Copy,
    TryFromPrimitive,
    IntoPrimitive,
)]
#[repr(u64)]
enum BlockType {
    PAYLOAD = 1,
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum Block {
    PAYLOAD(PayloadBlock),
    UNKOWN(UnkownBlock),
}

#[derive(Debug)]
pub struct CanonicalBlock {
    pub block: Block,
    pub block_number: u64,
    pub block_flags: BlockFlags,
    pub crc: CRCType,
}

impl Serialize for CanonicalBlock {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let len = if self.crc == CRCType::NoCRC { 5 } else { 6 };
        let mut seq = serializer.serialize_seq(Some(len))?;
        match &self.block {
            Block::PAYLOAD(_) => {
                let blocktype: u64 = BlockType::PAYLOAD.into();
                seq.serialize_element(&blocktype)?;
            }
            Block::UNKOWN(b) => {
                seq.serialize_element(&b.block_type)?;
            }
        }
        seq.serialize_element(&self.block_number)?;
        seq.serialize_element(&self.block_flags)?;
        seq.serialize_element(&self.crc)?;
        match &self.block {
            Block::PAYLOAD(b) => {
                seq.serialize_element(&b)?;
            }
            Block::UNKOWN(b) => {
                seq.serialize_element(&b)?;
            }
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

impl<'de> Deserialize<'de> for CanonicalBlock {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct BlockVisitor;
        impl<'de> Visitor<'de> for BlockVisitor {
            type Value = CanonicalBlock;

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

                let block_type_num: u64 = seq
                    .next_element()?
                    .ok_or(Error::custom("Error for field 'block_type'"))?;
                let block_type = BlockType::try_from(block_type_num);

                let block_number = seq
                    .next_element()?
                    .ok_or(Error::custom("Error for field 'block_number'"))?;
                let block_flags = seq
                    .next_element()?
                    .ok_or(Error::custom("Error for field 'block_flags'"))?;
                let mut crc: CRCType = seq
                    .next_element()?
                    .ok_or(Error::custom("Error for field 'crc_type'"))?;

                let data_bytes: &[u8] = seq
                    .next_element()?
                    .ok_or(Error::custom("Error for field 'data'"))?;
                let data: Vec<u8> = Vec::from(data_bytes);
                let block = match &block_type {
                    Ok(BlockType::PAYLOAD) => Block::PAYLOAD(PayloadBlock { data }),
                    Err(_) => Block::UNKOWN(UnkownBlock {
                        block_type: block_type_num,
                        data,
                    }),
                };

                if size == 6 {
                    crc = crc.deserialize_value(seq)?;
                }

                return Ok(CanonicalBlock {
                    block,
                    block_number,
                    block_flags,
                    crc,
                });
            }
        }
        deserializer.deserialize_seq(BlockVisitor)
    }
}

impl Validate for CanonicalBlock {
    fn validate(&self) -> bool {
        /*if !self.block.validate() {
            return false;
        }*/
        // TODO
        return true;
    }
}
