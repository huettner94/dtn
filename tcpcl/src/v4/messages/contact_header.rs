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
