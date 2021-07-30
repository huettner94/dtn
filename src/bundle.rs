use serde::{de::Error, de::Visitor, ser::SerializeSeq, Deserialize, Serialize};

use crate::{primaryblock::PrimaryBlock, Validate};

#[derive(Debug)]
pub struct Bundle {
    pub primary_block: PrimaryBlock,
}

impl Serialize for Bundle {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut seq = serializer.serialize_seq(None)?;
        seq.serialize_element(&self.primary_block)?;
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
                let primary_block = seq
                    .next_element()?
                    .ok_or(Error::custom("Error for field 'primary_block'"))?;

                return Ok(Bundle { primary_block });
            }
        }
        deserializer.deserialize_seq(BundleVisitor)
    }
}

impl Validate for Bundle {
    fn validate(&self) -> bool {
        return self.primary_block.validate();
    }
}
