use std::convert::TryInto;

use bytes::Buf;
use bytes::BufMut;
use bytes::BytesMut;
use num_enum::IntoPrimitive;
use num_enum::TryFromPrimitive;

use bitflags::bitflags;

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
