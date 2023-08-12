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

use bytes::{Buf, BufMut, BytesMut};

use crate::v4::messages::xfer_segment::MessageFlags;

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct XferAck {
    flags: MessageFlags,
    pub transfer_id: u64,
    pub acknowleged_length: u64,
}

impl XferAck {
    pub fn new(flags: MessageFlags, transfer_id: u64, acknowleged_length: u64) -> Self {
        XferAck {
            flags,
            transfer_id,
            acknowleged_length,
        }
    }

    pub fn decode(src: &mut BytesMut) -> Result<Option<Self>, crate::v4::messages::Errors> {
        if src.remaining() < 9 {
            return Ok(None);
        }

        let flags = src.get_u8();
        let transfer_id = src.get_u64();
        let acknowleged_length = src.get_u64();

        Ok(Some(XferAck {
            flags: MessageFlags::from_bits_truncate(flags),
            transfer_id,
            acknowleged_length,
        }))
    }

    pub fn encode(&self, dst: &mut BytesMut) {
        dst.reserve(9);
        dst.put_u8(self.flags.bits());
        dst.put_u64(self.transfer_id);
        dst.put_u64(self.acknowleged_length);
    }
}
