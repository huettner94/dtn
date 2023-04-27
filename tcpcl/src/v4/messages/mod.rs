use bytes::Buf;
use num_enum::IntoPrimitive;
use num_enum::TryFromPrimitive;

use tokio_util::codec::{Decoder, Encoder};

use bytes::{BufMut, BytesMut};

use self::contact_header::ContactHeader;
use self::keepalive::Keepalive;
use self::msg_reject::MsgReject;
use self::sess_init::SessInit;
use self::sess_term::SessTerm;
use self::xfer_ack::XferAck;
use self::xfer_refuse::XferRefuse;
use self::xfer_segment::XferSegment;

pub mod contact_header;
pub mod keepalive;
pub mod msg_reject;
pub mod sess_init;
pub mod sess_term;
pub mod xfer_ack;
pub mod xfer_refuse;
pub mod xfer_segment;

#[derive(Debug)]
pub enum Messages {
    ContactHeader(ContactHeader),
    SessInit(SessInit),
    SessTerm(SessTerm),
    Keepalive(Keepalive),
    MsgReject(MsgReject),
    XferSegment(XferSegment),
    XferAck(XferAck),
    XferRefuse(XferRefuse),
}

impl Messages {
    pub fn get_message_type(&self) -> MessageType {
        match self {
            Messages::ContactHeader(_) => panic!(),
            Messages::SessInit(_) => MessageType::SessInit,
            Messages::SessTerm(_) => MessageType::SessTerm,
            Messages::Keepalive(_) => MessageType::Keepalive,
            Messages::MsgReject(_) => MessageType::MsgReject,
            Messages::XferSegment(_) => MessageType::XferSegment,
            Messages::XferAck(_) => MessageType::XferAck,
            Messages::XferRefuse(_) => MessageType::XferRefuse,
        }
    }
}

#[derive(Debug, Eq, PartialEq, TryFromPrimitive, IntoPrimitive, Clone, Copy)]
#[repr(u8)]
pub enum MessageType {
    SessInit = 0x07,
    SessTerm = 0x05,
    XferSegment = 0x01,
    XferAck = 0x02,
    XferRefuse = 0x03,
    Keepalive = 0x04,
    MsgReject = 0x06,
}

#[derive(Debug)]
pub enum Errors {
    IoError(std::io::Error),
    InvalidMessageType(u8),
    InvalidHeader,
    UnkownCriticalSessionExtension(u16),
    UnkownCriticalTransferExtension(u16),
    SegmentTooLong,
    NodeIdInvalid,
}

impl From<std::io::Error> for Errors {
    fn from(e: std::io::Error) -> Self {
        Errors::IoError(e)
    }
}

#[derive(Debug, Clone, Default)]
pub struct Codec {
    contact_header_done: bool,
    curr_message_type: Option<MessageType>,
}

impl Decoder for Codec {
    type Item = Messages;

    type Error = Errors;

    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        if !self.contact_header_done {
            let contact_header =
                ContactHeader::decode(src).map(|o| o.map(|m| Messages::ContactHeader(m)));
            if contact_header.is_ok() && contact_header.as_ref().unwrap().is_some() {
                self.contact_header_done = true;
            }
            return contact_header;
        }

        if src.len() == 0 {
            return Ok(None);
        };
        if self.curr_message_type.is_none() {
            let message_type = src.get_u8();
            match message_type.try_into() {
                Ok(mt) => {
                    self.curr_message_type = Some(mt);
                }
                Err(_) => return Err(Errors::InvalidMessageType(message_type)),
            }
        }

        let decoded = match self.curr_message_type.as_ref().unwrap() {
            MessageType::SessInit => {
                SessInit::decode(src).map(|o| o.map(|m| Messages::SessInit(m)))
            }
            MessageType::SessTerm => {
                SessTerm::decode(src).map(|o| o.map(|m| Messages::SessTerm(m)))
            }
            MessageType::XferSegment => {
                XferSegment::decode(src).map(|o| o.map(|m| Messages::XferSegment(m)))
            }
            MessageType::XferAck => XferAck::decode(src).map(|o| o.map(|m| Messages::XferAck(m))),
            MessageType::XferRefuse => {
                XferRefuse::decode(src).map(|o| o.map(|m| Messages::XferRefuse(m)))
            }
            MessageType::Keepalive => {
                Keepalive::decode(src).map(|o| o.map(|m| Messages::Keepalive(m)))
            }
            MessageType::MsgReject => {
                MsgReject::decode(src).map(|o| o.map(|m| Messages::MsgReject(m)))
            }
        };

        if decoded.is_ok() && decoded.as_ref().unwrap().is_some() {
            self.curr_message_type = None;
        }
        decoded
    }
}

impl Encoder<Messages> for Codec {
    type Error = std::io::Error;

    fn encode(&mut self, item: Messages, dst: &mut BytesMut) -> Result<(), Self::Error> {
        match item {
            Messages::ContactHeader(m) => m.encode(dst),
            Messages::SessInit(m) => {
                dst.put_u8(MessageType::SessInit.into());
                m.encode(dst);
            }
            Messages::SessTerm(m) => {
                dst.put_u8(MessageType::SessTerm.into());
                m.encode(dst);
            }
            Messages::Keepalive(m) => {
                dst.put_u8(MessageType::Keepalive.into());
                m.encode(dst);
            }
            Messages::MsgReject(m) => {
                dst.put_u8(MessageType::MsgReject.into());
                m.encode(dst);
            }
            Messages::XferSegment(m) => {
                dst.put_u8(MessageType::XferSegment.into());
                m.encode(dst);
            }
            Messages::XferAck(m) => {
                dst.put_u8(MessageType::XferAck.into());
                m.encode(dst);
            }
            Messages::XferRefuse(m) => {
                dst.put_u8(MessageType::XferRefuse.into());
                m.encode(dst);
            }
        }
        Ok(())
    }
}
