use num_enum::IntoPrimitive;
use num_enum::TryFromPrimitive;

use self::contact_header::ContactHeader;
use self::sess_init::SessInit;
use self::sess_term::SessTerm;

pub mod contact_header;
pub mod reader;
pub mod sess_init;
pub mod sess_term;
pub mod statemachine;
pub mod transform;

#[derive(Debug)]
pub enum Messages {
    ContactHeader(ContactHeader),
    SessInit(SessInit),
    SessTerm(SessTerm),
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
