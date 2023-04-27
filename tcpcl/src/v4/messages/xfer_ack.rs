use bytes::{Buf, BufMut, BytesMut};

use crate::v4::messages::xfer_segment::MessageFlags;

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct XferAck {
    flags: MessageFlags,
    pub transfer_id: u64,
    pub acknowleged_length: u64,
}

impl XferAck {
    pub fn new(flags: MessageFlags, transfer_id: u64, acknowleged_length: u64) -> Self {
        XferAck {
            flags,
            transfer_id,
            acknowleged_length,
        }
    }

    pub fn decode(src: &mut BytesMut) -> Result<Option<Self>, crate::v4::messages::Errors> {
        if src.remaining() < 9 {
            return Ok(None);
        }

        let flags = src.get_u8();
        let transfer_id = src.get_u64();
        let acknowleged_length = src.get_u64();

        Ok(Some(XferAck {
            flags: MessageFlags::from_bits_truncate(flags),
            transfer_id,
            acknowleged_length,
        }))
    }

    pub fn encode(&self, dst: &mut BytesMut) {
        dst.reserve(9);
        dst.put_u8(self.flags.bits());
        dst.put_u64(self.transfer_id);
        dst.put_u64(self.acknowleged_length);
    }
}
