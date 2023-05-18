use bytes::Buf;
use bytes::BufMut;
use bytes::BytesMut;

use bitflags::bitflags;

const KEEPALIVE_DEFAULT_INTERVAL: u16 = 60;
pub const MAX_SEGMENT_MRU: u64 = 100 * 1024;
pub const MAX_TRANSFER_MRU: u64 = 1024 * 1024;

bitflags! {
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
    struct SessionExtensionFlags: u8 {
        const CRITICAL = 0x01;
    }
}

#[derive(Debug, Clone)]
pub struct SessionExtension {
    flags: SessionExtensionFlags,
    extension_type: u16,
    value: Vec<u8>,
}

impl SessionExtension {
    pub fn decode(src: &mut BytesMut) -> Result<Self, crate::v4::messages::Errors> {
        let flags = src.get_u8();
        let extension_type = src.get_u16();

        let value_length = src.get_u16();
        let value = src.get(0..value_length as usize).unwrap().to_vec();
        src.advance(value_length as usize);

        Ok(SessionExtension {
            flags: SessionExtensionFlags::from_bits_truncate(flags),
            extension_type,
            value,
        })
    }

    fn write(&self, target: &mut Vec<u8>) {
        target.reserve(5 + self.value.len());
        target.push(self.flags.bits());
        target.extend_from_slice(&self.extension_type.to_be_bytes());
        target.extend_from_slice(&(self.value.len() as u16).to_be_bytes());
        target.extend_from_slice(&self.value);
    }
}

#[derive(Debug, Clone)]
pub struct SessInit {
    pub keepalive_interval: u16,
    pub segment_mru: u64,
    pub transfer_mru: u64,
    pub node_id: String,
    pub session_extensions: Vec<SessionExtension>,
}

impl SessInit {
    pub fn new(node_id: String) -> Self {
        SessInit {
            keepalive_interval: KEEPALIVE_DEFAULT_INTERVAL,
            segment_mru: MAX_SEGMENT_MRU,
            transfer_mru: MAX_TRANSFER_MRU,
            node_id,
            session_extensions: Vec::new(),
        }
    }

    pub fn decode(src: &mut BytesMut) -> Result<Option<Self>, crate::v4::messages::Errors> {
        if src.remaining() < 24 {
            return Ok(None);
        }

        // We can not use get here, since we MUST NOT advance the cursor of `src` until we are sure
        // we can read a full frame
        let mut min_size: usize = 2 + 8 + 8 + 2 + 4;
        let node_id_length = u16::from_be_bytes(src[18..20].try_into().unwrap());
        min_size += node_id_length as usize;
        let session_extensions_length = u32::from_be_bytes(
            src[20 + node_id_length as usize..24 + node_id_length as usize]
                .try_into()
                .unwrap(),
        );
        min_size += session_extensions_length as usize;
        if src.remaining() < min_size {
            return Ok(None);
        }

        // from now on we can assume we have a full frame and the cursor is at the start of the frame

        let keepalive_interval = src.get_u16();
        let segment_mru = src.get_u64();
        let transfer_mru = src.get_u64();

        src.advance(2); // this is the node_id_length we read previously
        let node_id_vec = src.get(0..node_id_length as usize).unwrap().to_vec();
        src.advance(node_id_length as usize);
        let node_id = String::from_utf8(node_id_vec)
            .map_err(|_| crate::v4::messages::Errors::NodeIdInvalid)?;

        src.advance(4); // this is the session-extensions_length we read previously
        let mut session_extensions: Vec<SessionExtension> = Vec::new();
        let target_remaining = src.remaining() - session_extensions_length as usize;
        while src.remaining() > target_remaining {
            let se = SessionExtension::decode(src)?;
            if se.flags.contains(SessionExtensionFlags::CRITICAL) {
                return Err(crate::v4::messages::Errors::UnkownCriticalSessionExtension(
                    se.extension_type,
                ));
            }
            session_extensions.push(se);
        }

        Ok(Some(SessInit {
            keepalive_interval,
            segment_mru,
            transfer_mru,
            node_id,
            session_extensions,
        }))
    }

    pub fn encode(&self, dst: &mut BytesMut) {
        dst.reserve(24 + self.node_id.len() + self.session_extensions.len() * 5);
        dst.put_u16(self.keepalive_interval);
        dst.put_u64(self.segment_mru);
        dst.put_u64(self.transfer_mru);
        dst.put_u16(self.node_id.len() as u16);
        dst.put(self.node_id.as_bytes());

        let mut session_extension_bytes: Vec<u8> = Vec::new();
        for session_extension in &self.session_extensions {
            session_extension.write(&mut session_extension_bytes);
        }
        dst.put_u32(session_extension_bytes.len() as u32);
        dst.put(&session_extension_bytes[..]);
    }
}
