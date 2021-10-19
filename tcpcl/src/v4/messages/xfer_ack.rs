use crate::errors::Errors;
use crate::v4::{messages::xfer_segment::MessageFlags, reader::Reader, transform::Transform};

#[derive(Debug, PartialEq, Eq)]
pub struct XferAck {
    flags: MessageFlags,
    transfer_id: u64,
    acknowleged_length: u64,
}

impl XferAck {
    pub fn new(flags: MessageFlags, transfer_id: u64, acknowleged_length: u64) -> Self {
        XferAck {
            flags,
            transfer_id,
            acknowleged_length,
        }
    }
}

impl Transform for XferAck {
    fn read(reader: &mut Reader) -> Result<Self, Errors>
    where
        Self: Sized,
    {
        if reader.left() < 9 {
            return Err(Errors::MessageTooShort);
        }
        let flags = reader.read_u8();
        let transfer_id = reader.read_u64();
        let acknowleged_length = reader.read_u64();

        Ok(XferAck {
            flags: MessageFlags::from_bits_truncate(flags),
            transfer_id,
            acknowleged_length,
        })
    }

    fn write(&self, target: &mut Vec<u8>) {
        target.reserve(9);
        target.push(self.flags.bits());
        target.extend_from_slice(&self.transfer_id.to_be_bytes());
        target.extend_from_slice(&self.acknowleged_length.to_be_bytes());
    }
}
