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

use bytes::Buf;
use bytes::BufMut;
use bytes::BytesMut;
use num_enum::IntoPrimitive;
use num_enum::TryFromPrimitive;

#[derive(Debug, Eq, PartialEq, Clone, Copy, TryFromPrimitive, IntoPrimitive)]
#[repr(u8)]
pub enum ReasonCode {
    Unkown = 0x00,
    MessageTypeUnkown = 0x01,
    MessageUnsupported = 0x02,
    MessageUnexpected = 0x03,
}

#[derive(Debug)]
pub struct MsgReject {
    pub reason: ReasonCode,
    message_header: u8,
}

impl MsgReject {
    pub fn new(reason: ReasonCode, message_header: u8) -> Self {
        MsgReject {
            reason,
            message_header,
        }
    }

    pub fn decode(src: &mut BytesMut) -> Result<Option<Self>, crate::v4::messages::Errors> {
        if src.remaining() < 2 {
            return Ok(None);
        }

        let reason = src.get_u8().try_into().unwrap_or(ReasonCode::Unkown);
        let message_header = src.get_u8();

        Ok(Some(MsgReject {
            reason,
            message_header,
        }))
    }

    pub fn encode(&self, dst: &mut BytesMut) {
        dst.reserve(2);
        dst.put_u8(self.reason.into());
        dst.put_u8(self.message_header);
    }
}
