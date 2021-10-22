use std::{cmp::min, convert::TryInto, mem};

use log::{info, warn};
use tokio::io::Interest;

use crate::{errors::Errors, transfer::Transfer};

use super::{
    messages::{
        contact_header::ContactHeader,
        sess_init::SessInit,
        sess_term::{ReasonCode, SessTerm},
        xfer_ack::XferAck,
        xfer_segment::{self, XferSegment},
        MessageType, Messages,
    },
    reader::Reader,
    transform::Transform,
};

#[derive(Debug, PartialEq, Eq)]
struct TransferTracker {
    transfer: Transfer,
    pos: usize,
    pos_acked: usize,
}

#[derive(Debug, PartialEq, Eq)]
enum States {
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
    // Data Transfer (Receiving)
    SendXferAck(XferAck),
    // Data Transfer (Sending)
    SendXferSegments(TransferTracker),
    // Data Transfer (both),
    SendXferSegmentsAndAck(TransferTracker, XferAck),
    // Session Termination
    SendSessTerm(Option<ReasonCode>),
    WaitSessTerm,
    // Final
    ConnectionClose,

    // Impelementation Detail
    ShouldNeverExist,
}

#[derive(Debug)]
pub struct StateMachine {
    state: States,
    my_contact_header: Option<ContactHeader>,
    peer_contact_header: Option<ContactHeader>,
    my_sess_init: Option<SessInit>,
    peer_sess_init: Option<SessInit>,
    terminating: bool,
}

impl StateMachine {
    pub fn new_active() -> Self {
        StateMachine {
            state: States::ActiveSendContactHeader,
            my_contact_header: None,
            peer_contact_header: None,
            my_sess_init: None,
            peer_sess_init: None,
            terminating: false,
        }
    }
    pub fn new_passive() -> Self {
        StateMachine {
            state: States::PassiveWaitContactHeader,
            my_contact_header: None,
            peer_contact_header: None,
            my_sess_init: None,
            peer_sess_init: None,
            terminating: false,
        }
    }

    pub fn send_message(&mut self, writer: &mut Vec<u8>) {
        match &mut self.state {
            States::ActiveSendContactHeader | States::PassiveSendContactHeader => {
                let ch = ContactHeader::new();
                self.my_contact_header = Some(ch.clone());
                ch.write(writer);
            }
            States::ActiveSendSessInit | States::PassiveSendSessInit => {
                let si = SessInit::new();
                self.my_sess_init = Some(si.clone());
                writer.push(MessageType::SessInit.into());
                si.write(writer);
            }
            States::SendXferAck(xfer_ack) | States::SendXferSegmentsAndAck(_, xfer_ack) => {
                writer.push(MessageType::XferAck.into());
                xfer_ack.write(writer);
            }
            States::SendSessTerm(r) => {
                let st = SessTerm::new(r.unwrap_or(ReasonCode::Unkown), self.terminating);
                writer.push(MessageType::SessTerm.into());
                st.write(writer);
            }
            States::SendXferSegments(tt) => {
                let mru = self.peer_sess_init.as_ref().unwrap().segment_mru;
                let end_pos = min(tt.pos + mru as usize, tt.transfer.data.len());
                if tt.pos == tt.transfer.data.len() {
                    warn!("We should not try to send a transfer if we already sent all data. We just dont do anything");
                    return;
                }
                let mut data = Vec::with_capacity(end_pos - tt.pos);
                data.extend_from_slice(&tt.transfer.data[tt.pos..end_pos]);

                let mut flags = xfer_segment::MessageFlags::empty();
                if tt.pos == 0 {
                    flags |= xfer_segment::MessageFlags::START;
                }
                if end_pos == tt.transfer.data.len() {
                    flags |= xfer_segment::MessageFlags::END;
                }

                let xfer_seg = XferSegment::new(flags, tt.transfer.id, data);
                writer.push(MessageType::XferSegment.into());
                xfer_seg.write(writer);
                tt.pos = end_pos;
            }
            _ => {
                panic!(
                    "Tried to send a message while we should be receiving. State: {:?}",
                    self.state
                );
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
            | States::SessionEstablished
            | States::SendXferSegments(_)
            | States::SendXferSegmentsAndAck(_, _) => {
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
                        self.peer_sess_init = Some(si.clone());
                        self.state = States::SessionEstablished;
                        Ok(Messages::SessInit(si))
                    }
                    MessageType::SessInit if self.state == States::PassiveWaitSessInit => {
                        let si = SessInit::read(reader)?;
                        self.peer_sess_init = Some(si.clone());
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
                    MessageType::XferAck => {
                        let xa = XferAck::read(reader)?;
                        match &mut self.state {
                            States::SendXferSegments(tt)
                            | States::SendXferSegmentsAndAck(tt, _) => {
                                if tt.transfer.id != xa.transfer_id {
                                    panic!("Remote send ack for transfer {}, but we are currently sending {}", xa.transfer_id, tt.transfer.id);
                                }
                                tt.pos_acked = xa.acknowleged_length as usize;
                                if tt.pos_acked == tt.transfer.data.len() {
                                    info!("Transfer {} finished (sent and acked)", tt.transfer.id);
                                    let state =
                                        mem::replace(&mut self.state, States::ShouldNeverExist);
                                    match state {
                                        States::SendXferSegments(_) => {
                                            self.state = States::SessionEstablished;
                                        }
                                        States::SendXferSegmentsAndAck(_, ack) => {
                                            self.state = States::SendXferAck(ack);
                                        }
                                        _ => return Err(Errors::MessageTypeInappropriate),
                                    }
                                }
                                Ok(Messages::XferAck(xa))
                            }
                            _ => Err(Errors::MessageTypeInappropriate),
                        }
                    }
                    _ => Err(Errors::MessageTypeInappropriate),
                }
            }
            _ => {
                panic!(
                    "Tried to decode a message while we should be sending. State: {:?}",
                    self.state
                );
            }
        }
    }

    pub fn get_interests(&self) -> Interest {
        match &self.state {
            States::ActiveSendContactHeader
            | States::PassiveSendContactHeader
            | States::ActiveSendSessInit
            | States::PassiveSendSessInit
            | States::SendXferAck(_)
            | States::SendSessTerm(_)
            | States::SendXferSegmentsAndAck(_, _) => Interest::WRITABLE,
            States::PassiveWaitContactHeader
            | States::ActiveWaitContactHeader
            | States::ActiveWaitSessInit
            | States::PassiveWaitSessInit
            | States::SessionEstablished
            | States::WaitSessTerm => Interest::READABLE,
            States::SendXferSegments(tt) => {
                if tt.pos < tt.transfer.data.len() {
                    return Interest::READABLE | Interest::WRITABLE;
                }
                Interest::READABLE
            }
            States::ConnectionClose => {
                panic!("Tried to continue after connection should be closed")
            }
            States::ShouldNeverExist => {
                panic!("Reached a state that should never exist")
            }
        }
    }

    pub fn send_complete(&mut self) {
        let state = mem::replace(&mut self.state, States::ShouldNeverExist);
        match state {
            States::ActiveSendContactHeader => self.state = States::ActiveWaitContactHeader,
            States::PassiveSendContactHeader => self.state = States::PassiveWaitSessInit,
            States::ActiveSendSessInit => self.state = States::ActiveWaitSessInit,
            States::PassiveSendSessInit | States::SendXferAck(_) => {
                self.state = States::SessionEstablished
            }
            States::SendXferSegmentsAndAck(tt, _) => {
                // We here rely on the fact that send_message will prefer
                // acks over xfers
                self.state = States::SendXferSegments(tt);
            }
            States::SendXferSegments(tt) => {
                // This will probably never happen, but just to be sure
                if tt.pos == tt.transfer.data.len() && tt.pos == tt.pos_acked {
                    self.state = States::SessionEstablished;
                } else {
                    self.state = States::SendXferSegments(tt);
                }
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

    pub fn send_transfer(&mut self, transfer: Transfer) {
        let tracker = TransferTracker {
            transfer,
            pos: 0,
            pos_acked: 0,
        };
        let state = mem::replace(&mut self.state, States::ShouldNeverExist);
        match state {
            States::SendXferAck(ack) => self.state = States::SendXferSegmentsAndAck(tracker, ack),
            States::SessionEstablished => self.state = States::SendXferSegments(tracker),
            _ => {
                panic!("Attempted to send a transfer on a non-established connection");
            }
        }
    }

    pub fn send_ack(&mut self, ack: XferAck) {
        let state = mem::replace(&mut self.state, States::ShouldNeverExist);
        match state {
            States::SendXferSegments(tt) => self.state = States::SendXferSegmentsAndAck(tt, ack),
            States::SessionEstablished => self.state = States::SendXferAck(ack),
            _ => {
                panic!("Attempted to send an ack on a non-established connection");
            }
        }
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
        self.state == States::SessionEstablished
    }

    pub fn could_send_transfer(&self) -> bool {
        self.state == States::SessionEstablished
    }

    pub fn should_close(&self) -> bool {
        self.state == States::ConnectionClose
    }
}
