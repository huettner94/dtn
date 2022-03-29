use bitflags::bitflags;

use crate::errors::Errors;
use crate::v4::reader::Reader;
use crate::v4::transform::Transform;

use super::xfer_ack::XferAck;

bitflags! {
    struct TransferExtensionFlags: u8 {
        const CRITICAL = 0x01;
    }
}

#[derive(Debug)]
pub struct TransferExtension {
    flags: TransferExtensionFlags,
    extension_type: u16,
    value: Vec<u8>,
}

impl Transform for TransferExtension {
    fn read(reader: &mut Reader) -> Result<Self, Errors>
    where
        Self: Sized,
    {
        if reader.left() < 5 {
            return Err(Errors::MessageTooShort);
        }
        let flags = reader.read_u8();
        let extension_type = reader.read_u16();

        let value_length = reader.read_u16();
        if reader.left() < value_length.into() {
            return Err(Errors::MessageTooShort);
        }
        let mut value: Vec<u8> = vec![0; value_length as usize];
        reader.read_u8_array(&mut value[..], value_length.into());

        Ok(TransferExtension {
            flags: TransferExtensionFlags::from_bits_truncate(flags),
            extension_type,
            value,
        })
    }

    fn write(&self, target: &mut Vec<u8>) {
        target.reserve(5 + self.value.len());
        target.push(self.flags.bits);
        target.extend_from_slice(&self.extension_type.to_be_bytes());
        target.extend_from_slice(&(self.value.len() as u16).to_be_bytes());
        target.extend_from_slice(&self.value);
    }
}

bitflags! {
    pub struct MessageFlags: u8 {
        const END = 0x01;
        const START = 0x02;
    }
}

#[derive(Debug)]
pub struct XferSegment {
    pub flags: MessageFlags,
    pub transfer_id: u64,
    transfer_extensions: Vec<TransferExtension>,
    pub data: Vec<u8>,
}

impl XferSegment {
    pub fn new(flags: MessageFlags, transfer_id: u64, data: Vec<u8>) -> Self {
        XferSegment {
            flags,
            transfer_id,
            transfer_extensions: Vec::new(),
            data,
        }
    }

    pub fn to_xfer_ack(&self, acknowleged_length: u64) -> XferAck {
        XferAck::new(self.flags, self.transfer_id, acknowleged_length)
    }
}

impl Transform for XferSegment {
    fn read(reader: &mut Reader) -> Result<Self, Errors>
    where
        Self: Sized,
    {
        if reader.left() < 17 {
            return Err(Errors::MessageTooShort);
        }
        let flags = MessageFlags::from_bits_truncate(reader.read_u8());
        let transfer_id = reader.read_u64();

        let mut transfer_extensions: Vec<TransferExtension> = Vec::new();
        if flags.contains(MessageFlags::START) {
            if reader.left() < 12 {
                return Err(Errors::MessageTooShort);
            }
            let transfer_extensions_length = reader.read_u32();
            if reader.left() < transfer_extensions_length as usize {
                return Err(Errors::MessageTooShort);
            }
            let target_reader_pos = reader.current_pos() + transfer_extensions_length as usize;
            while reader.current_pos() < target_reader_pos {
                let se = TransferExtension::read(reader)?;
                if se.flags.contains(TransferExtensionFlags::CRITICAL) {
                    return Err(Errors::UnkownCriticalTransferExtension(se.extension_type));
                }
                transfer_extensions.push(se);
            }
        }

        let data_length = reader.read_u64();
        if data_length > super::sess_init::MAX_SEGMENT_MRU {
            return Err(Errors::SegmentTooLong);
        }
        if reader.left() < (data_length as usize) {
            return Err(Errors::MessageTooShort);
        }
        let mut data: Vec<u8> = vec![0; data_length as usize];
        reader.read_u8_array(&mut data[..], data_length as usize);

        Ok(XferSegment {
            flags,
            transfer_id,
            transfer_extensions,
            data,
        })
    }

    fn write(&self, target: &mut Vec<u8>) {
        target.reserve(21 + self.data.len() + self.transfer_extensions.len() * 5);
        target.push(self.flags.bits);
        target.extend_from_slice(&self.transfer_id.to_be_bytes());

        if self.flags.contains(MessageFlags::START) {
            let mut transfer_extension_bytes: Vec<u8> = Vec::new();
            for transfer_extension in &self.transfer_extensions {
                transfer_extension.write(&mut transfer_extension_bytes);
            }
            target.extend_from_slice(&(transfer_extension_bytes.len() as u32).to_be_bytes());
            target.extend_from_slice(&transfer_extension_bytes);
        }

        target.extend_from_slice(&(self.data.len() as u64).to_be_bytes());
        target.extend_from_slice(&self.data);
    }
}
