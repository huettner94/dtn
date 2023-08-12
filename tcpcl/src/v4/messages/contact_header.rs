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

use bitflags::bitflags;
use bytes::{Buf, BufMut, BytesMut};

bitflags! {
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
    pub struct ContactHeaderFields: u8 {
        const CAN_TLS = 0x01;
    }
}

const DTN_MAGIC_BYTES: [u8; 4] = [0x64, 0x74, 0x6E, 0x21];

#[derive(Debug, Clone)]
pub struct ContactHeader {
    magic: [u8; 4],
    version: u8,
    flags: ContactHeaderFields,
}

impl ContactHeader {
    pub fn new(can_tls: bool) -> Self {
        let mut flags = ContactHeaderFields::empty();
        if can_tls {
            flags |= ContactHeaderFields::CAN_TLS;
        }
        ContactHeader {
            magic: DTN_MAGIC_BYTES,
            version: 4,
            flags,
        }
    }

    pub fn can_tls(&self) -> bool {
        self.flags.contains(ContactHeaderFields::CAN_TLS)
    }

    pub fn decode(src: &mut BytesMut) -> Result<Option<Self>, crate::v4::messages::Errors> {
        if src.remaining() < 6 {
            return Ok(None);
        }

        let magic: [u8; 4] = src.get(0..4).unwrap().try_into().unwrap();
        src.advance(4);
        if magic != DTN_MAGIC_BYTES {
            return Err(crate::v4::messages::Errors::InvalidHeader);
        }
        let version = src.get_u8();
        let flags = src.get_u8();
        Ok(Some(ContactHeader {
            magic,
            version,
            flags: ContactHeaderFields::from_bits_truncate(flags),
        }))
    }

    pub fn encode(&self, dst: &mut BytesMut) {
        dst.reserve(6);
        dst.put_slice(&self.magic);
        dst.put_u8(self.version);
        dst.put_u8(self.flags.bits());
    }
}
