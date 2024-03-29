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

use std::{
    cmp::{max, min},
    convert::{TryFrom, TryInto},
    fmt::Write,
    ops::ControlFlow,
};

use binascii::hex2bin;
use serde::{de::Error, de::Visitor, ser::SerializeSeq, Deserialize, Serialize};

use crate::{
    block::{Block, CanonicalBlock},
    blockflags::BlockFlags,
    bundleflags::BundleFlags,
    primaryblock::PrimaryBlock,
    FragmentationError, SerializationError, Validate,
};

use super::block::payload_block::PayloadBlock;

const BUNDLE_SERIALIZATION_OVERHEAD: u64 = 2; // 1 byte for the start of the cbor list and 1 byte for the end
                                              // for block with the highest possibe values for all fields + CRC32 is 41 bytes.
                                              // we need to account for the payload length value encoding as well. To be safe we go to 128 bytes in total.
const PAYLOAD_BLOCK_SERIALIZATION_OVERHEAD: u64 = 128;

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct Bundle {
    pub primary_block: PrimaryBlock,
    pub blocks: Vec<CanonicalBlock>,
}

impl Serialize for Bundle {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut seq = serializer.serialize_seq(None)?;
        seq.serialize_element(&self.primary_block)?;
        for block in &self.blocks {
            seq.serialize_element(&block)?;
        }
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
                let mut blocks: Vec<CanonicalBlock> = match seq.size_hint() {
                    Some(v) => Vec::with_capacity(v),
                    None => Vec::new(),
                };
                let primary_block = seq
                    .next_element()?
                    .ok_or(Error::custom("Error for field 'primary_block'"))?;
                while let Some(block) = seq.next_element()? {
                    blocks.push(block);
                }

                if blocks.is_empty() {
                    return Err(Error::invalid_length(0, &"must have at least one block"));
                }

                Ok(Bundle {
                    primary_block,
                    blocks,
                })
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
        true
    }
}

impl TryFrom<Vec<u8>> for Bundle {
    type Error = SerializationError;

    fn try_from(value: Vec<u8>) -> Result<Self, Self::Error> {
        serde_cbor::from_slice(&value).map_err(SerializationError::SerializationError)
    }
}

impl TryFrom<Bundle> for Vec<u8> {
    type Error = SerializationError;

    fn try_from(value: Bundle) -> Result<Self, Self::Error> {
        (&value).try_into()
    }
}

impl TryFrom<&Bundle> for Vec<u8> {
    type Error = SerializationError;

    fn try_from(value: &Bundle) -> Result<Self, Self::Error> {
        serde_cbor::to_vec(value).map_err(SerializationError::SerializationError)
    }
}

impl Bundle {
    pub fn as_hex(&self) -> Result<String, SerializationError> {
        let vec: Vec<u8> = self.try_into()?;
        let mut s = String::with_capacity(2 * vec.len());
        for b in vec {
            write!(&mut s, "{:02X?}", &b).or_else(|_| Err(SerializationError::ConversionError))?;
        }
        Ok(s)
    }

    pub fn from_hex(hex: &str) -> Result<Bundle, SerializationError> {
        let mut val = vec![0; hex.len() / 2];
        hex2bin(hex.as_bytes(), &mut val).unwrap();
        val.try_into()
    }

    fn payload_canonical_block(&self) -> &CanonicalBlock {
        for block in &self.blocks {
            if let Block::Payload(_) = &block.block {
                return block;
            }
        }
        panic!("All Bundles MUST contain a payload block");
    }

    pub fn payload_block(&self) -> &PayloadBlock {
        match &self.payload_canonical_block().block {
            Block::Payload(p) => p,
            _ => panic!("The payload block is always the payload block"),
        }
    }

    fn mut_payload_block(&mut self) -> &mut PayloadBlock {
        for block in &mut self.blocks {
            if let Block::Payload(p) = &mut block.block {
                return p;
            }
        }
        panic!("All Bundles MUST contain a payload block");
    }

    pub fn fragment(self, max_size: u64) -> Result<(Vec<Bundle>, u64, u64), FragmentationError> {
        if Vec::<u8>::try_from(&self)?.len() as u64 <= max_size {
            panic!(
                "Fragmentation not needed, bundle already smaller than {}",
                max_size
            );
        }
        if self
            .primary_block
            .bundle_processing_flags
            .contains(BundleFlags::MUST_NOT_FRAGMENT)
        {
            return Err(FragmentationError::MustNotFragment);
        }
        if self
            .primary_block
            .bundle_processing_flags
            .contains(BundleFlags::FRAGMENT)
            && (self.primary_block.fragment_offset.is_none()
                || self.primary_block.total_data_length.is_none())
        {
            return Err(FragmentationError::BundleInvalid);
        }

        let primary_block_size = serde_cbor::to_vec(&self.primary_block)?.len() as u64;
        let mut first_fragment_min_size = primary_block_size
            + PAYLOAD_BLOCK_SERIALIZATION_OVERHEAD
            + BUNDLE_SERIALIZATION_OVERHEAD;
        let mut fragment_min_size = first_fragment_min_size;
        for block in &self.blocks {
            if matches!(block.block, Block::Payload(_)) {
                continue;
            }
            let block_size = serde_cbor::to_vec(block)?.len() as u64;
            first_fragment_min_size += block_size;
            if block
                .block_flags
                .contains(BlockFlags::MUST_REPLICATE_TO_ALL_FRAGMENTS)
            {
                fragment_min_size += block_size;
            }
        }
        if first_fragment_min_size > max_size || fragment_min_size > max_size {
            return Err(FragmentationError::CanNotFragmentThatSmall(
                first_fragment_min_size,
            ));
        }

        let mut fragments = Vec::new();
        let mut current_payload_offset: u64 = 0;
        let payload_length = self.payload_block().data.len() as u64;

        let global_payload_offset = self.primary_block.fragment_offset.unwrap_or(0); // 0 if the bundle was no fragment before
        let total_data_length = self
            .primary_block
            .total_data_length
            .unwrap_or(payload_length); // default if the bundle was no fragment before

        let new_primary_block = PrimaryBlock {
            bundle_processing_flags: self.primary_block.bundle_processing_flags
                | BundleFlags::FRAGMENT,
            fragment_offset: Some(0),
            total_data_length: Some(total_data_length),
            ..self.primary_block.clone()
        };

        let first_fragment_blocks = self
            .blocks
            .iter()
            .filter(|b| !matches!(b.block, Block::Payload(_)))
            .cloned()
            .collect::<Vec<_>>();

        let fragment_blocks = self
            .blocks
            .iter()
            .filter(|b| {
                !matches!(b.block, Block::Payload(_))
                    && !b
                        .block_flags
                        .contains(BlockFlags::MUST_REPLICATE_TO_ALL_FRAGMENTS)
            })
            .cloned()
            .collect::<Vec<_>>();

        let current_payload_canonical_block = self.payload_canonical_block();
        let payload_canonical_block = CanonicalBlock {
            block: Block::Payload(PayloadBlock { data: Vec::new() }),
            block_flags: current_payload_canonical_block.block_flags,
            block_number: current_payload_canonical_block.block_number,
            crc: current_payload_canonical_block.crc,
        };

        while current_payload_offset < payload_length {
            let mut fragment = Bundle {
                primary_block: PrimaryBlock {
                    fragment_offset: Some(global_payload_offset + current_payload_offset),
                    ..new_primary_block.clone()
                },
                blocks: if current_payload_offset == 0 {
                    first_fragment_blocks.clone()
                } else {
                    fragment_blocks.clone()
                },
            };

            let fragment_size = if current_payload_offset == 0 {
                first_fragment_min_size
            } else {
                fragment_min_size
            };

            let payload_length_for_fragment = min(
                payload_length - current_payload_offset,
                max_size - fragment_size,
            );

            if payload_length_for_fragment < 1 {
                panic!("Would create a bundle with a payload block of size 0");
            }

            let payload_block = PayloadBlock {
                data: self.payload_block().data[current_payload_offset as usize
                    ..(current_payload_offset + payload_length_for_fragment) as usize]
                    .to_vec(),
            };
            fragment.blocks.push(CanonicalBlock {
                block: Block::Payload(payload_block),
                ..payload_canonical_block.clone()
            });

            let fragment_length = Vec::<u8>::try_from(&fragment)?.len() as u64;
            if fragment_length > max_size {
                panic!("Attempted to fragment bundle to size {} but built a fragment of size {}. This is a bug", max_size, fragment_length);
            }

            fragments.push(fragment);
            current_payload_offset += payload_length_for_fragment;
        }

        Ok((fragments, first_fragment_min_size, fragment_min_size))
    }

    pub fn can_reassemble_bundles(bundles: &mut Vec<&Bundle>) -> bool {
        if bundles.is_empty() {
            return false;
        }
        let first = &bundles[0];
        if !first
            .primary_block
            .bundle_processing_flags
            .contains(BundleFlags::FRAGMENT)
        {
            panic!("Tried to reassemble a bundle that is not a fragment");
        }

        if !bundles.iter().all(|item| {
            first
                .primary_block
                .equals_ignoring_fragment_offset(&item.primary_block)
        }) {
            panic!("Tried to reassemble bundles with different primary blocks. They probably belong to different bundles");
        }

        let total_data_length = bundles[0].primary_block.total_data_length.unwrap();

        bundles.sort_by(|a, b| {
            a.primary_block
                .fragment_offset
                .unwrap()
                .cmp(&b.primary_block.fragment_offset.unwrap())
        });
        if bundles[0].primary_block.fragment_offset.unwrap() != 0 {
            return false;
        }
        let is_continiuous = match bundles
            .iter()
            .map(|b| {
                (
                    b.primary_block.fragment_offset.unwrap(),
                    b.payload_block().data.len() as u64,
                )
            })
            .try_fold(0, |acc, (offset, len)| {
                if offset < acc {
                    // We have some range duplicated, but that should not be an issue
                    return ControlFlow::Continue(max(acc, offset + len));
                }
                if acc != offset {
                    ControlFlow::Break(false)
                } else {
                    ControlFlow::Continue(offset + len)
                }
            }) {
            ControlFlow::Continue(len) => len == total_data_length,
            ControlFlow::Break(_) => false,
        };
        if !is_continiuous {
            return false;
        }

        true
    }

    pub fn reassemble_bundles(mut bundles: Vec<&Bundle>) -> Option<Bundle> {
        if !Bundle::can_reassemble_bundles(&mut bundles) {
            return None;
        }

        let total_data_length = bundles[0].primary_block.total_data_length.unwrap();

        let mut main_bundle = bundles.drain(0..1).next().unwrap().clone();
        main_bundle
            .primary_block
            .bundle_processing_flags
            .remove(BundleFlags::FRAGMENT);
        main_bundle.primary_block.fragment_offset = None;
        main_bundle.primary_block.total_data_length = None;

        let payload_block = main_bundle.mut_payload_block();
        payload_block
            .data
            .reserve_exact((total_data_length as usize) - payload_block.data.len());
        let mut current_len = payload_block.data.len();
        for bundle in bundles {
            let fragment_offset = bundle.primary_block.fragment_offset.unwrap() as usize;
            if fragment_offset + bundle.payload_block().data.len() < current_len {
                continue;
            }
            let start = current_len - fragment_offset;
            payload_block
                .data
                .extend_from_slice(&bundle.payload_block().data[start..]);
            current_len = payload_block.data.len();
        }

        Some(main_bundle)
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        block::{
            hop_count_block::HopCountBlock, payload_block::PayloadBlock, Block, CanonicalBlock,
        },
        blockflags::BlockFlags,
        bundleflags::BundleFlags,
        crc::CRCType,
        endpoint::Endpoint,
        primaryblock::PrimaryBlock,
        time::{CreationTimestamp, DtnTime},
        FragmentationError,
    };

    use super::Bundle;

    fn get_bundle_data() -> Vec<u8> {
        let mut data: Vec<u8> = Vec::new();
        for i in 0..1024 {
            data.push(i as u8);
        }
        data
    }

    fn get_test_bundle() -> Bundle {
        let data = get_bundle_data();
        Bundle {
            primary_block: PrimaryBlock {
                version: 7,
                bundle_processing_flags: BundleFlags::BUNDLE_DELIVERY_STATUS_REQUESTED,
                crc: CRCType::NoCRC,
                destination_endpoint: Endpoint::new("dtn://node31/mavlink").unwrap(),
                source_node: Endpoint::new("dtn://node2/incoming").unwrap(),
                report_to: Endpoint::new("dtn://node2/incoming").unwrap(),
                creation_timestamp: CreationTimestamp {
                    creation_time: DtnTime {
                        timestamp: 681253789438,
                    },
                    sequence_number: 0,
                },
                lifetime: 3600000,
                fragment_offset: None,
                total_data_length: None,
            },
            blocks: [
                CanonicalBlock {
                    block: Block::HopCount(HopCountBlock {
                        limit: 32,
                        count: 0,
                    }),
                    block_number: 2,
                    block_flags: BlockFlags::empty(),
                    crc: CRCType::NoCRC,
                },
                CanonicalBlock {
                    block: Block::Payload(PayloadBlock { data }),
                    block_number: 1,
                    block_flags: BlockFlags::empty(),
                    crc: CRCType::NoCRC,
                },
            ]
            .into(),
        }
    }

    #[test]
    fn fragment_bundle() -> Result<(), FragmentationError> {
        let fragments = get_test_bundle().fragment(256)?.0;
        let mut current_offset = 0;
        for fragment in &fragments {
            assert!(fragment
                .primary_block
                .bundle_processing_flags
                .contains(BundleFlags::FRAGMENT));
            assert_eq!(fragment.primary_block.total_data_length.unwrap(), 1024);
            let fragment_length = Vec::<u8>::try_from(fragment)?.len() as u64;
            assert!(fragment_length <= 256);
            let offset = fragment.primary_block.fragment_offset.unwrap();
            let length = fragment.payload_block().data.len() as u64;
            assert_eq!(offset, current_offset);
            current_offset += length;
        }
        assert_eq!(
            current_offset,
            fragments[0].primary_block.total_data_length.unwrap()
        );
        assert_eq!(fragments.len(), 23);
        Ok(())
    }

    #[test]
    fn double_fragment_bundle() -> Result<(), FragmentationError> {
        let mut fragments_first = get_test_bundle().fragment(750)?.0;
        let fragments: Vec<Bundle> = fragments_first
            .drain(0..fragments_first.len())
            .flat_map(|f| f.fragment(600).unwrap().0)
            .collect();

        let mut current_offset = 0;
        for fragment in &fragments {
            let fragment_length = Vec::<u8>::try_from(fragment)?.len() as u64;
            assert!(fragment_length <= 600);
            let offset = fragment.primary_block.fragment_offset.unwrap();
            let length = fragment.payload_block().data.len() as u64;
            assert_eq!(offset, current_offset);
            current_offset += length;
        }
        assert_eq!(
            current_offset,
            fragments[0].primary_block.total_data_length.unwrap()
        );
        assert_eq!(fragments.len(), 4);
        Ok(())
    }

    #[test]
    fn reassembly_bundle_2_frags() -> Result<(), FragmentationError> {
        let fragments = get_test_bundle().fragment(800)?.0;
        assert_eq!(fragments.len(), 2);
        let fragments_ref = fragments.iter().collect();

        let reassembled = &Bundle::reassemble_bundles(fragments_ref).unwrap();

        assert!(!reassembled
            .primary_block
            .bundle_processing_flags
            .contains(BundleFlags::FRAGMENT));
        assert!(reassembled.primary_block.fragment_offset.is_none());
        assert!(reassembled.primary_block.total_data_length.is_none());
        assert_eq!(reassembled.payload_block().data.len(), 1024);
        assert_eq!(reassembled.payload_block().data, get_bundle_data());

        Ok(())
    }

    #[test]
    fn reassmeble_double_fragment_bundle() -> Result<(), FragmentationError> {
        let mut fragments_first = get_test_bundle().fragment(750)?.0;
        let mut fragments: Vec<Bundle> = fragments_first
            .drain(0..fragments_first.len())
            .flat_map(|f| f.fragment(600).unwrap().0)
            .collect();

        let mut current_offset = 0;
        for fragment in &fragments {
            let fragment_length = Vec::<u8>::try_from(fragment)?.len() as u64;
            assert!(fragment_length <= 600);
            let offset = fragment.primary_block.fragment_offset.unwrap();
            let length = fragment.payload_block().data.len() as u64;
            assert_eq!(offset, current_offset);
            current_offset += length;
        }
        assert_eq!(
            current_offset,
            fragments[0].primary_block.total_data_length.unwrap()
        );
        assert_eq!(fragments.len(), 4);

        // just to test reordering
        fragments.swap(0, 2);
        fragments.swap(1, 3);
        let fragments_ref = fragments.iter().collect();

        let reassembled_bundles = Bundle::reassemble_bundles(fragments_ref);
        assert!(reassembled_bundles.is_some());
        let reassembled = reassembled_bundles.unwrap();
        assert!(reassembled.primary_block.fragment_offset.is_none());
        assert!(reassembled.primary_block.total_data_length.is_none());
        assert_eq!(reassembled.payload_block().data.len(), 1024);
        assert_eq!(reassembled.payload_block().data, get_bundle_data());

        Ok(())
    }

    #[test]
    fn reassembly_bundle_overlap() -> Result<(), FragmentationError> {
        let mut fragments = get_test_bundle().fragment(800)?.0;
        assert_eq!(fragments.len(), 2);
        let tmpdata = fragments[1].payload_block().data[0];
        fragments[0].mut_payload_block().data.push(tmpdata);

        let fragments_ref = fragments.iter().collect();

        let reassembled = &Bundle::reassemble_bundles(fragments_ref).unwrap();

        assert!(!reassembled
            .primary_block
            .bundle_processing_flags
            .contains(BundleFlags::FRAGMENT));
        assert!(reassembled.primary_block.fragment_offset.is_none());
        assert!(reassembled.primary_block.total_data_length.is_none());
        assert_eq!(reassembled.payload_block().data.len(), 1024);
        assert_eq!(reassembled.payload_block().data, get_bundle_data());

        Ok(())
    }
}
