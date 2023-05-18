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
