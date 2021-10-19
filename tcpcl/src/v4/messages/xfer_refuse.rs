use std::convert::TryInto;

use num_enum::IntoPrimitive;
use num_enum::TryFromPrimitive;

use crate::errors::Errors;
use crate::v4::reader::Reader;
use crate::v4::transform::Transform;

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
}

impl Transform for XferRefuse {
    fn read(reader: &mut Reader) -> Result<Self, Errors>
    where
        Self: Sized,
    {
        if reader.left() < 5 {
            return Err(Errors::MessageTooShort);
        }
        let reason = reader.read_u8();
        let transfer_id = reader.read_u64();

        Ok(XferRefuse {
            reason: reason.try_into().or(Ok(ReasonCode::Unkown))?,
            transfer_id,
        })
    }

    fn write(self, target: &mut Vec<u8>) {
        target.reserve(5);
        target.push(self.reason.into());
        target.extend_from_slice(&self.transfer_id.to_be_bytes());
    }
}
