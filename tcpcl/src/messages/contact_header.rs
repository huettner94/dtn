use bitflags::bitflags;

use crate::errors::Errors;

use super::{reader::Reader, transform::Transform};

bitflags! {
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
    pub fn new() -> Self {
        ContactHeader {
            magic: DTN_MAGIC_BYTES,
            version: 4,
            flags: ContactHeaderFields::empty(),
        }
    }
}

impl Transform for ContactHeader {
    fn read(reader: &mut Reader) -> Result<Self, Errors>
    where
        Self: Sized,
    {
        if reader.left() < 6 {
            return Err(Errors::MessageTooShort);
        }
        let mut magic: [u8; 4] = [0; 4];
        reader.read_u8_array(&mut magic, 4);
        if magic != DTN_MAGIC_BYTES {
            return Err(Errors::InvalidHeader);
        }
        let version = reader.read_u8();
        let flags = reader.read_u8();
        Ok(ContactHeader {
            magic,
            version,
            flags: ContactHeaderFields::from_bits_truncate(flags),
        })
    }

    fn write(self, target: &mut Vec<u8>) {
        target.reserve(6);
        target.extend_from_slice(&self.magic);
        target.push(self.version);
        target.push(self.flags.bits);
    }
}
