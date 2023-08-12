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

use bitflags::bitflags;

bitflags! {
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
    struct MessageFlags: u8 {
        const REPLY = 0x01;
    }
}

#[derive(Debug, Eq, PartialEq, Clone, Copy, TryFromPrimitive, IntoPrimitive)]
#[repr(u8)]
pub enum ReasonCode {
    Unkown = 0x00,
    IdleTimeout = 0x01,
    VersionMissmatch = 0x02,
    Busy = 0x03,
    ContactFailure = 0x04,
    ResourceExhaustion = 0x05,
}

#[derive(Debug)]
pub struct SessTerm {
    flags: MessageFlags,
    pub reason: ReasonCode,
}

impl SessTerm {
    pub fn new(reason: ReasonCode, reply: bool) -> Self {
        let flags = match reply {
            true => MessageFlags::REPLY,
            false => MessageFlags::empty(),
        };
        SessTerm { flags, reason }
    }

    pub fn decode(src: &mut BytesMut) -> Result<Option<Self>, crate::v4::messages::Errors> {
        if src.remaining() < 2 {
            return Ok(None);
        }

        let flags = src.get_u8();
        let reason = src.get_u8();

        Ok(Some(SessTerm {
            flags: MessageFlags::from_bits_truncate(flags),
            reason: reason.try_into().unwrap_or(ReasonCode::Unkown),
        }))
    }

    pub fn encode(&self, dst: &mut BytesMut) {
        dst.reserve(2);
        dst.put_u8(self.flags.bits());
        dst.put_u8(self.reason.into());
    }
}
