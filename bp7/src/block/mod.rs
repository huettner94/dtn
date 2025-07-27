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

use std::convert::TryFrom;

use serde::{Deserialize, Serialize, de::Error, de::Visitor, ser::SerializeSeq};

use crate::{blockflags::BlockFlags, crc::CRCType, *};

use self::bundle_age_block::BundleAgeBlock;
use self::hop_count_block::HopCountBlock;
use self::previous_node_block::PreviousNodeBlock;
use self::{payload_block::PayloadBlock, unkown_block::UnkownBlock};
use num_enum::IntoPrimitive;
use num_enum::TryFromPrimitive;
use serde_repr::{Deserialize_repr, Serialize_repr};

pub mod bundle_age_block;
pub mod hop_count_block;
pub mod payload_block;
pub mod previous_node_block;
pub mod unkown_block;

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
    Payload = 1,
    PreviousNode = 6,
    BundleAge = 7,
    HopCount = 10,
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum Block {
    Payload(PayloadBlock),
    PreviousNode(PreviousNodeBlock),
    BundleAge(BundleAgeBlock),
    HopCount(HopCountBlock),
    Unkown(UnkownBlock),
}

#[derive(Debug, PartialEq, Eq, Clone)]
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
        let blocktype: u64 = match &self.block {
            Block::Payload(_) => BlockType::Payload.into(),
            Block::PreviousNode(_) => BlockType::PreviousNode.into(),
            Block::BundleAge(_) => BlockType::BundleAge.into(),
            Block::HopCount(_) => BlockType::HopCount.into(),
            Block::Unkown(b) => b.block_type,
        };
        seq.serialize_element(&blocktype)?;
        seq.serialize_element(&self.block_number)?;
        seq.serialize_element(&self.block_flags)?;
        seq.serialize_element(&self.crc)?;
        match &self.block {
            Block::Payload(b) => {
                seq.serialize_element(&b)?;
            }
            Block::PreviousNode(b) => {
                seq.serialize_element(&b)?;
            }
            Block::BundleAge(b) => {
                seq.serialize_element(&b)?;
            }
            Block::HopCount(b) => {
                seq.serialize_element(&b)?;
            }
            Block::Unkown(b) => {
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
                let size = seq.size_hint().ok_or_else(|| {
                    Error::custom("Canonical Block must know the length of its contents")
                })?;
                if !(5..=6).contains(&size) {
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
                    Ok(BlockType::Payload) => Block::Payload(PayloadBlock { data }),
                    Ok(BlockType::PreviousNode) => Block::PreviousNode(PreviousNodeBlock { data }),
                    Ok(BlockType::BundleAge) => {
                        Block::BundleAge(BundleAgeBlock::try_from(data).map_err(Error::custom)?)
                    }
                    Ok(BlockType::HopCount) => {
                        Block::HopCount(HopCountBlock::try_from(data).map_err(Error::custom)?)
                    }
                    Err(_) => Block::Unkown(UnkownBlock {
                        block_type: block_type_num,
                        data,
                    }),
                };

                if size == 6 {
                    crc = crc.deserialize_value(seq)?;
                }

                Ok(CanonicalBlock {
                    block,
                    block_number,
                    block_flags,
                    crc,
                })
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
        true
    }
}
