use serde::{de::Error, de::Visitor, ser::SerializeSeq, Deserialize, Serialize};

use crate::{block::Block, primaryblock::PrimaryBlock, Validate};

#[derive(Debug)]
pub struct Bundle {
    pub primary_block: PrimaryBlock,
    pub blocks: Vec<Block>,
}

impl Serialize for Bundle {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut seq = serializer.serialize_seq(None)?;
        seq.serialize_element(&self.primary_block)?;
        seq.serialize_element(&self.blocks)?;
        seq.end()
    }
}

impl<'de> Deserialize<'de> for Bundle {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct BundleVisitor;
        impl<'de> Visitor<'de> for BundleVisitor {
            type Value = Bundle;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("bundle")
            }

            fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
            where
                A: serde::de::SeqAccess<'de>,
            {
                let mut blocks: Vec<Block> = match seq.size_hint() {
                    Some(v) => Vec::with_capacity(v),
                    None => Vec::new(),
                };
                let primary_block = seq
                    .next_element()?
                    .ok_or(Error::custom("Error for field 'primary_block'"))?;
                while let Some(block) = seq.next_element()? {
                    blocks.push(block);
                }

                if blocks.len() < 1 {
                    return Err(Error::invalid_length(0, &"must have at least one block"));
                }

                return Ok(Bundle {
                    primary_block,
                    blocks,
                });
            }
        }
        deserializer.deserialize_seq(BundleVisitor)
    }
}

impl Validate for Bundle {
    fn validate(&self) -> bool {
        if !self.primary_block.validate() {
            return false;
        }
        for block in &self.blocks {
            if !block.validate() {
                return false;
            }
        }
        return true;
    }
}
