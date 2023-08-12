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

use bytes::{Buf, BufMut, BytesMut};
use num_enum::IntoPrimitive;
use num_enum::TryFromPrimitive;

#[derive(Debug, Eq, PartialEq, Clone, Copy, TryFromPrimitive, IntoPrimitive)]
#[repr(u8)]
pub enum ReasonCode {
    Unkown = 0x00,
    Completed = 0x01,
    NoResources = 0x02,
    Retransmit = 0x03,
    NotAcceptable = 0x04,
    ExtensionFailure = 0x05,
    SessionTerminating = 0x06,
}

#[derive(Debug)]
pub struct XferRefuse {
    reason: ReasonCode,
    transfer_id: u64,
}

impl XferRefuse {
    pub fn new(transfer_id: u64) -> Self {
        XferRefuse {
            reason: ReasonCode::Unkown,
            transfer_id,
        }
    }

    pub fn decode(src: &mut BytesMut) -> Result<Option<Self>, crate::v4::messages::Errors> {
        if src.remaining() < 5 {
            return Ok(None);
        }

        let reason = src.get_u8().try_into().unwrap_or(ReasonCode::Unkown);
        let transfer_id = src.get_u64();

        Ok(Some(XferRefuse {
            reason,
            transfer_id,
        }))
    }

    pub fn encode(&self, dst: &mut BytesMut) {
        dst.reserve(5);
        dst.put_u8(self.reason.into());
        dst.put_u64(self.transfer_id);
    }
}
