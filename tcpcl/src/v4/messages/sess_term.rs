use std::convert::TryInto;

use num_enum::IntoPrimitive;
use num_enum::TryFromPrimitive;

use bitflags::bitflags;

use crate::errors::Errors;
use crate::v4::reader::Reader;
use crate::v4::transform::Transform;

bitflags! {
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
}

impl Transform for SessTerm {
    fn read(reader: &mut Reader) -> Result<Self, Errors>
    where
        Self: Sized,
    {
        if reader.left() < 2 {
            return Err(Errors::MessageTooShort);
        }
        let flags = reader.read_u8();
        let reason = reader.read_u8();

        Ok(SessTerm {
            flags: MessageFlags::from_bits_truncate(flags),
            reason: reason.try_into().or(Ok(ReasonCode::Unkown))?,
        })
    }

    fn write(&self, target: &mut Vec<u8>) {
        target.reserve(2);
        target.push(self.flags.bits);
        target.push(self.reason.into());
    }
}
