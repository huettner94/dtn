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

use std::fmt::Debug;

use bitflags::bitflags;
use bytes::{Buf, BufMut, BytesMut};

use super::xfer_ack::XferAck;

bitflags! {
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
    struct TransferExtensionFlags: u8 {
        const CRITICAL = 0x01;
    }
}

#[derive(Debug)]
pub struct TransferExtension {
    flags: TransferExtensionFlags,
    extension_type: u16,
    value: Vec<u8>,
}

impl TransferExtension {
    pub fn decode(src: &mut BytesMut) -> Result<Self, crate::v4::messages::Errors> {
        let flags = src.get_u8();
        let extension_type = src.get_u16();

        let value_length = src.get_u16();
        assert!(src.remaining() >= value_length.into());
        let mut value: Vec<u8> = vec![0; value_length as usize];
        value.put(src.take(value_length.into()));

        Ok(TransferExtension {
            flags: TransferExtensionFlags::from_bits_truncate(flags),
            extension_type,
            value,
        })
    }

    fn write(&self, target: &mut Vec<u8>) {
        target.reserve(5 + self.value.len());
        target.push(self.flags.bits());
        target.extend_from_slice(&self.extension_type.to_be_bytes());
        target.extend_from_slice(&u16::try_from(self.value.len()).unwrap().to_be_bytes());
        target.extend_from_slice(&self.value);
    }
}

bitflags! {
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
    pub struct MessageFlags: u8 {
        const END = 0x01;
        const START = 0x02;
    }
}

pub struct XferSegment {
    pub flags: MessageFlags,
    pub transfer_id: u64,
    transfer_extensions: Vec<TransferExtension>,
    pub data: Vec<u8>,
}

impl Debug for XferSegment {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("XferSegment")
            .field("flags", &self.flags)
            .field("transfer_id", &self.transfer_id)
            .field("transfer_extensions", &self.transfer_extensions)
            .field("data (length)", &self.data.len())
            .finish()
    }
}

impl XferSegment {
    pub fn new(flags: MessageFlags, transfer_id: u64, data: Vec<u8>) -> Self {
        XferSegment {
            flags,
            transfer_id,
            transfer_extensions: Vec::new(),
            data,
        }
    }

    pub fn to_xfer_ack(&self, acknowleged_length: u64) -> XferAck {
        XferAck::new(self.flags, self.transfer_id, acknowleged_length)
    }

    pub fn decode(src: &mut BytesMut) -> Result<Option<Self>, crate::v4::messages::Errors> {
        if src.remaining() < 10 {
            return Ok(None);
        }

        // We can not use get here, since we MUST NOT advance the cursor of `src` until we are sure
        // we can read a full frame
        let flags = MessageFlags::from_bits_truncate(src[0]);

        // This is just to ensure we have the full frame
        let mut min_size: usize = 1 + 8 + 8;
        if flags.contains(MessageFlags::START) {
            min_size += 4; // for the transfer extension items length
            min_size += u32::from_be_bytes(src[9..13].try_into().unwrap()) as usize;
            if src.remaining() < min_size {
                return Ok(None);
            }
        }

        let data_length =
            u64::from_be_bytes(src[min_size - 8..min_size].try_into().unwrap()) as usize;
        if data_length > super::sess_init::MAX_SEGMENT_MRU {
            return Err(crate::v4::messages::Errors::SegmentTooLong);
        }

        min_size += data_length;
        if src.remaining() < min_size {
            return Ok(None);
        }
        src.advance(1);

        // from now on we can assume we have a full frame and the cursor is directly after the message flags
        let transfer_id = src.get_u64();

        let mut transfer_extensions: Vec<TransferExtension> = Vec::new();
        if flags.contains(MessageFlags::START) {
            assert!(src.remaining() >= 12);
            let transfer_extensions_length = src.get_u32();
            assert!(src.remaining() >= transfer_extensions_length as usize);
            let target_remaining = src.remaining() - transfer_extensions_length as usize;
            while src.remaining() > target_remaining {
                let se = TransferExtension::decode(src)?;
                if se.flags.contains(TransferExtensionFlags::CRITICAL) {
                    return Err(
                        crate::v4::messages::Errors::UnkownCriticalTransferExtension(
                            se.extension_type,
                        ),
                    );
                }
                transfer_extensions.push(se);
            }
        }

        src.advance(8); // for data length we read above
        assert!(src.remaining() >= data_length);
        let mut data = vec![0; data_length];
        src.copy_to_slice(&mut data);

        Ok(Some(XferSegment {
            flags,
            transfer_id,
            transfer_extensions,
            data,
        }))
    }

    pub fn encode(&self, dst: &mut BytesMut) {
        dst.reserve(21 + self.data.len() + self.transfer_extensions.len() * 5);
        dst.put_u8(self.flags.bits());
        dst.put_u64(self.transfer_id);

        if self.flags.contains(MessageFlags::START) {
            let mut transfer_extension_bytes: Vec<u8> = Vec::new();
            for transfer_extension in &self.transfer_extensions {
                transfer_extension.write(&mut transfer_extension_bytes);
            }
            dst.put_u32(transfer_extension_bytes.len().try_into().unwrap());
            dst.extend_from_slice(&transfer_extension_bytes);
        }

        dst.put_u64(self.data.len().try_into().unwrap());
        dst.extend_from_slice(&self.data);
    }
}
