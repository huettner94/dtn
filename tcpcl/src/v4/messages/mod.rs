use num_enum::IntoPrimitive;
use num_enum::TryFromPrimitive;

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

#[derive(Debug, Eq, PartialEq, TryFromPrimitive, IntoPrimitive)]
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
