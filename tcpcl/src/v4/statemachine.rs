use std::{cmp::min, convert::TryInto, mem};

use log::{info, warn};
use tokio::io::Interest;

use crate::{errors::Errors, transfer::Transfer};

use super::{
    messages::{
        contact_header::ContactHeader,
        keepalive::Keepalive,
        msg_reject::{self, MsgReject},
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
    // Keepalive
    SendKeepalive(Box<States>),
    // Session Termination
    SendSessTerm(Option<ReasonCode>),
    WaitSessTerm,
    // Rejects (peer errors)
    SendMsgReject(msg_reject::ReasonCode, u8),
    // Final
    ConnectionClose,

    // Impelementation Detail
    ShouldNeverExist,
}

#[derive(Debug)]
pub struct StateMachine {
    state: States,
    my_node_id: String,
    last_used_transfer_id: u64,
    my_contact_header: Option<ContactHeader>,
    peer_contact_header: Option<ContactHeader>,
    my_sess_init: Option<SessInit>,
    peer_sess_init: Option<SessInit>,
    terminating: bool,
}

impl StateMachine {
    pub fn new_active(node_id: String) -> Self {
        StateMachine {
            state: States::ActiveSendContactHeader,
            my_node_id: node_id,
            last_used_transfer_id: 0,
            my_contact_header: None,
            peer_contact_header: None,
            my_sess_init: None,
            peer_sess_init: None,
            terminating: false,
        }
    }
    pub fn new_passive(node_id: String) -> Self {
        StateMachine {
            state: States::PassiveWaitContactHeader,
            my_node_id: node_id,
            last_used_transfer_id: 0,
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
                let si = SessInit::new(self.my_node_id.clone());
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
            States::SendKeepalive(_) => {
                writer.push(MessageType::Keepalive.into());
                Keepalive::new().write(writer);
            }
            States::SendMsgReject(r, t) => {
                writer.push(MessageType::MsgReject.into());
                MsgReject::new(*r, *t).write(writer);
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
            | States::SendXferSegmentsAndAck(_, _)
            | States::SendKeepalive(_) => {
                if reader.left() < 1 {
                    return Err(Errors::MessageTooShort);
                }
                let message_type_num = reader.read_u8();
                let message_type: MessageType = message_type_num.try_into().map_err(|_| {
                    self.state = States::SendMsgReject(
                        msg_reject::ReasonCode::MessageTypeUnkown,
                        message_type_num,
                    );
                    Errors::UnkownMessageType
                })?;
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
                    MessageType::Keepalive => {
                        let k = Keepalive::read(reader)?;
                        Ok(Messages::Keepalive(k))
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
                                        _ => panic!("Invalid state {:?}", state),
                                    }
                                }
                                Ok(Messages::XferAck(xa))
                            }
                            _ => {
                                self.state = States::SendMsgReject(
                                    msg_reject::ReasonCode::MessageUnexpected,
                                    MessageType::XferAck.into(),
                                );
                                Err(Errors::MessageTypeInappropriate)
                            }
                        }
                    }
                    MessageType::MsgReject => {
                        let rej = MsgReject::read(reader)?;
                        Ok(Messages::MsgReject(rej))
                    }
                    _ => {
                        self.state = States::SendMsgReject(
                            msg_reject::ReasonCode::MessageUnexpected,
                            message_type.into(),
                        );
                        Err(Errors::MessageTypeInappropriate)
                    }
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
            | States::SendXferSegmentsAndAck(_, _)
            | States::SendKeepalive(_)
            | States::SendMsgReject(_, _) => Interest::WRITABLE,
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
            States::SendKeepalive(s) => {
                self.state = *s;
            }
            States::SendSessTerm(_) if self.terminating == true => {
                self.state = States::ConnectionClose;
            }
            States::SendSessTerm(_) if self.terminating == false => {
                self.terminating = true;
                self.state = States::WaitSessTerm
            }
            States::SendMsgReject(_, _) => {
                self.terminating = true;
                self.state = States::ConnectionClose;
            }
            _ => {
                panic!("{:?} is not a valid state to complete sending", state);
            }
        }
    }

    pub fn send_transfer(&mut self, data: Vec<u8>) {
        let tracker = TransferTracker {
            transfer: Transfer {
                id: self.last_used_transfer_id,
                data,
            },
            pos: 0,
            pos_acked: 0,
        };
        self.last_used_transfer_id += 1;
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

    pub fn send_keepalive(&mut self) {
        let state = mem::replace(&mut self.state, States::ShouldNeverExist);
        self.state = States::SendKeepalive(Box::new(state));
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

    pub fn get_peer_node_id(&self) -> String {
        if !self.is_established() {
            panic!("Attempted to get the peer node-id on a non-established connection");
        }
        self.peer_sess_init.as_ref().unwrap().node_id.clone()
    }

    pub fn get_keepalive_interval(&self) -> Option<u16> {
        if !self.is_established() {
            panic!("Attempted to get the keepalive interval on a non-established connection");
        }
        let my_keepalive = self.my_sess_init.as_ref().unwrap().keepalive_interval;
        let peer_keepalive = self.peer_sess_init.as_ref().unwrap().keepalive_interval;
        let keepalive = min(my_keepalive, peer_keepalive);
        match keepalive {
            0 => None,
            x => Some(x),
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
