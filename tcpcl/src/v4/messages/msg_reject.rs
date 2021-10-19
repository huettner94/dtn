use std::convert::TryInto;

use num_enum::IntoPrimitive;
use num_enum::TryFromPrimitive;

use crate::errors::Errors;
use crate::v4::reader::Reader;
use crate::v4::transform::Transform;

use super::MessageType;

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
    message_header: MessageType,
}

impl MsgReject {
    pub fn new(reason: ReasonCode, message_header: MessageType) -> Self {
        MsgReject {
            reason,
            message_header,
        }
    }
}

impl Transform for MsgReject {
    fn read(reader: &mut Reader) -> Result<Self, Errors>
    where
        Self: Sized,
    {
        if reader.left() < 2 {
            return Err(Errors::MessageTooShort);
        }
        let reason = reader.read_u8();
        let message_header = reader.read_u8();

        Ok(MsgReject {
            reason: reason.try_into().or(Ok(ReasonCode::Unkown))?,
            message_header: message_header
                .try_into()
                .or(Err(Errors::UnkownMessageType))?,
        })
    }

    fn write(self, target: &mut Vec<u8>) {
        target.reserve(2);
        target.push(self.reason.into());
        target.push(self.message_header.into());
    }
}
