use std::convert::TryInto;

use tokio::io::Interest;

use crate::errors::Errors;

use super::{
    messages::{
        contact_header::ContactHeader,
        sess_init::SessInit,
        sess_term::{ReasonCode, SessTerm},
        xfer_ack::XferAck,
        xfer_segment::XferSegment,
        MessageType, Messages,
    },
    reader::Reader,
    transform::Transform,
};

#[derive(Debug, PartialEq, Eq)]
pub enum States {
    // Handshake Part 1
    ActiveSendContactHeader,
    PassiveWaitContactHeader,
    // Handshake Part 2
    ActiveWaitContactHeader,
    PassiveSendContactHeader,
    // Session Initialization Part 1
    ActiveSendSessInit,
    PassiveWaitSessInit,
    // Session Initialization Part 2
    ActiveWaitSessInit,
    PassiveSendSessInit,
    // Session Established
    SessionEstablished,
    // Data Transfer
    SendXferAck(XferAck),
    // Session Termination
    SendSessTerm(Option<ReasonCode>),
    WaitSessTerm,
    // Final
    ConnectionClose,
}

#[derive(Debug)]
pub struct StateMachine {
    pub state: States,
    my_contact_header: Option<ContactHeader>,
    peer_contact_header: Option<ContactHeader>,
    terminating: bool,
}

impl StateMachine {
    pub fn new_active() -> Self {
        StateMachine {
            state: States::ActiveSendContactHeader,
            my_contact_header: None,
            peer_contact_header: None,
            terminating: false,
        }
    }
    pub fn new_passive() -> Self {
        StateMachine {
            state: States::PassiveWaitContactHeader,
            my_contact_header: None,
            peer_contact_header: None,
            terminating: false,
        }
    }

    pub fn send_message(&mut self, writer: &mut Vec<u8>) {
        match &self.state {
            States::ActiveSendContactHeader | States::PassiveSendContactHeader => {
                let ch = ContactHeader::new();
                self.my_contact_header = Some(ch.clone());
                ch.write(writer);
            }
            States::ActiveSendSessInit | States::PassiveSendSessInit => {
                let si = SessInit::new();
                writer.push(MessageType::SessInit.into());
                si.write(writer);
            }
            States::SendXferAck(xfer_ack) => {
                writer.push(MessageType::XferAck.into());
                xfer_ack.write(writer);
            }
            States::SendSessTerm(r) => {
                let st = SessTerm::new(r.unwrap_or(ReasonCode::Unkown), self.terminating);
                writer.push(MessageType::SessTerm.into());
                st.write(writer);
            }
            _ => {
                panic!("Tried to send a message while we should be receiving")
            }
        }
    }

    pub fn decode_message(&mut self, reader: &mut Reader) -> Result<Messages, Errors> {
        let out = self.decode_message_inner(reader);
        if out.is_ok() {
            reader.consume();
        } else {
            reader.reset_read();
        }
        return out;
    }

    fn decode_message_inner(&mut self, reader: &mut Reader) -> Result<Messages, Errors> {
        match self.state {
            States::PassiveWaitContactHeader | States::ActiveWaitContactHeader => {
                let ch = ContactHeader::read(reader)?;
                self.peer_contact_header = Some(ch.clone());
                if self.state == States::PassiveWaitContactHeader {
                    self.state = States::PassiveSendContactHeader;
                } else {
                    self.state = States::ActiveSendSessInit;
                }
                Ok(Messages::ContactHeader(ch))
            }
            States::ActiveWaitSessInit
            | States::PassiveWaitSessInit
            | States::WaitSessTerm
            | States::SessionEstablished => {
                if reader.left() < 1 {
                    return Err(Errors::MessageTooShort);
                }
                let message_type: MessageType = reader
                    .read_u8()
                    .try_into()
                    .map_err(|_| Errors::UnkownMessageType)?;
                match message_type {
                    MessageType::SessInit if self.state == States::ActiveWaitSessInit => {
                        let si = SessInit::read(reader)?;
                        self.state = States::SessionEstablished;
                        Ok(Messages::SessInit(si))
                    }
                    MessageType::SessInit if self.state == States::PassiveWaitSessInit => {
                        let si = SessInit::read(reader)?;
                        self.state = States::PassiveSendSessInit;
                        Ok(Messages::SessInit(si))
                    }
                    MessageType::SessTerm if self.state == States::WaitSessTerm => {
                        let st = SessTerm::read(reader)?;
                        self.state = States::ConnectionClose;
                        Ok(Messages::SessTerm(st))
                    }
                    MessageType::SessTerm if self.state == States::SessionEstablished => {
                        let st = SessTerm::read(reader)?;
                        self.state = States::SendSessTerm(Some(st.reason));
                        self.terminating = true;
                        Ok(Messages::SessTerm(st))
                    }
                    MessageType::XferSegment if self.state == States::SessionEstablished => {
                        let xs = XferSegment::read(reader)?;
                        Ok(Messages::XferSegment(xs))
                    }
                    _ => Err(Errors::MessageTypeInappropriate),
                }
            }
            _ => {
                panic!("Tried to decode a message while we should be sending")
            }
        }
    }

    pub fn get_interests(&self) -> Interest {
        match self.state {
            States::ActiveSendContactHeader
            | States::PassiveSendContactHeader
            | States::ActiveSendSessInit
            | States::PassiveSendSessInit
            | States::SendXferAck(_)
            | States::SendSessTerm(_) => Interest::WRITABLE,
            States::PassiveWaitContactHeader
            | States::ActiveWaitContactHeader
            | States::ActiveWaitSessInit
            | States::PassiveWaitSessInit
            | States::SessionEstablished
            | States::WaitSessTerm => Interest::READABLE,
            States::ConnectionClose => {
                panic!("Tried to continue after connection should be closed")
            }
        }
    }

    pub fn send_complete(&mut self) {
        match self.state {
            States::ActiveSendContactHeader => self.state = States::ActiveWaitContactHeader,
            States::PassiveSendContactHeader => self.state = States::PassiveWaitSessInit,
            States::ActiveSendSessInit => self.state = States::ActiveWaitSessInit,
            States::PassiveSendSessInit | States::SendXferAck(_) => {
                self.state = States::SessionEstablished
            }
            States::SendSessTerm(_) => {
                self.terminating = true;
                self.state = States::WaitSessTerm
            }
            _ => {
                panic!("{:?} is not a valid state to complete sending", self.state);
            }
        }
    }

    pub fn send_ack(&mut self, ack: XferAck) {
        if !self.is_established() {
            panic!("Attempted to send an ack on a non-established connection");
        }
        self.state = States::SendXferAck(ack);
    }

    pub fn close_connection(&mut self, reason: Option<ReasonCode>) {
        if !self.is_established() {
            panic!("Attempted to close a non-established connection");
        }
        self.state = States::SendSessTerm(reason);
    }

    pub fn connection_closing(&self) -> bool {
        match self.state {
            States::SendSessTerm(_) | States::WaitSessTerm | States::ConnectionClose => true,
            _ => false,
        }
    }

    pub fn is_established(&self) -> bool {
        return self.state == States::SessionEstablished;
    }

    pub fn should_close(&self) -> bool {
        return self.state == States::ConnectionClose;
    }
}