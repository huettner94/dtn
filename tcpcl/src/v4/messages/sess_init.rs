use crate::{
    errors::Errors,
    v4::{reader::Reader, transform::Transform},
};

use bitflags::bitflags;

bitflags! {
    struct SessionExtensionFlags: u8 {
        const CRITICAL = 0x01;
    }
}

#[derive(Debug)]
pub struct SessionExtension {
    flags: SessionExtensionFlags,
    extension_type: u16,
    value: Vec<u8>,
}

impl Transform for SessionExtension {
    fn read(reader: &mut Reader) -> Result<Self, Errors>
    where
        Self: Sized,
    {
        if reader.left() < 6 {
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

        Ok(SessionExtension {
            flags: SessionExtensionFlags::from_bits_truncate(flags),
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

#[derive(Debug)]
pub struct SessInit {
    keepalive_interval: u16,
    segment_mru: u64,
    transfer_mru: u64,
    node_id: String,
    session_extensions: Vec<SessionExtension>,
}

impl SessInit {
    pub fn new() -> Self {
        SessInit {
            keepalive_interval: 0,
            segment_mru: (crate::v4::reader::READER_BUFFER_SIZE - 1024) as u64, // Hopefully 1024 is enough for all future headers
            transfer_mru: 100000,
            node_id: "somestring".into(),
            session_extensions: Vec::new(),
        }
    }
}

impl Transform for SessInit {
    fn read(reader: &mut Reader) -> Result<Self, Errors>
    where
        Self: Sized,
    {
        if reader.left() < 24 {
            return Err(Errors::MessageTooShort);
        }
        let keepalive_interval = reader.read_u16();
        let segment_mru = reader.read_u64();
        let transfer_mru = reader.read_u64();

        let node_id_length = reader.read_u16();
        if reader.left() < (node_id_length as usize) + 4 {
            return Err(Errors::MessageTooShort);
        }
        let node_id = reader
            .read_string(node_id_length.into())
            .map_err(|_| Errors::NodeIdInvalid)?;

        let session_extensions_length = reader.read_u32();
        if reader.left() < session_extensions_length as usize {
            return Err(Errors::MessageTooShort);
        }
        let mut session_extensions: Vec<SessionExtension> = Vec::new();
        let target_reader_pos = reader.current_pos() + session_extensions_length as usize;
        while reader.current_pos() < target_reader_pos {
            let se = SessionExtension::read(reader)?;
            if se.flags.contains(SessionExtensionFlags::CRITICAL) {
                return Err(Errors::UnkownCriticalSessionExtension(se.extension_type));
            }
            session_extensions.push(se);
        }

        Ok(SessInit {
            keepalive_interval,
            segment_mru,
            transfer_mru,
            node_id,
            session_extensions,
        })
    }

    fn write(&self, target: &mut Vec<u8>) {
        target.reserve(24 + self.node_id.len() + self.session_extensions.len() * 5);
        target.extend_from_slice(&self.keepalive_interval.to_be_bytes());
        target.extend_from_slice(&self.segment_mru.to_be_bytes());
        target.extend_from_slice(&self.transfer_mru.to_be_bytes());
        target.extend_from_slice(&(self.node_id.len() as u16).to_be_bytes());
        target.extend_from_slice(self.node_id.as_bytes());

        let mut session_extension_bytes: Vec<u8> = Vec::new();
        for session_extension in &self.session_extensions {
            session_extension.write(&mut session_extension_bytes);
        }
        target.extend_from_slice(&(session_extension_bytes.len() as u32).to_be_bytes());
        target.extend_from_slice(&session_extension_bytes);
    }
}
