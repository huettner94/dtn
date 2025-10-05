// Copyright (C) 2023 Felix Huettner
//
// This file is part of DTRD.
//
// DTRD is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// DTRD is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with this program.  If not, see <https://www.gnu.org/licenses/>.

use std::{cmp::min, mem, pin::Pin, sync::Arc};

use log::{error, info, warn};
use tokio::io::{Interest, WriteHalf};
use tokio_util::codec::FramedWrite;

use crate::{
    errors::{Errors, TransferSendErrors},
    session::AsyncReadWrite,
    transfer::Transfer,
};
use futures_util::SinkExt;

use super::messages::{
    self, Codec, MessageType, Messages,
    contact_header::ContactHeader,
    keepalive::Keepalive,
    msg_reject::{self, MsgReject},
    sess_init::SessInit,
    sess_term::{ReasonCode, SessTerm},
    xfer_ack::XferAck,
    xfer_segment::{self, XferSegment},
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
    can_tls: bool,
    my_node_id: String,
    last_used_transfer_id: u64,
    my_contact_header: Option<ContactHeader>,
    peer_contact_header: Option<ContactHeader>,
    my_sess_init: Option<SessInit>,
    peer_sess_init: Option<SessInit>,
    terminating: bool,
}

impl StateMachine {
    pub fn new_active(node_id: String, can_tls: bool) -> Self {
        StateMachine {
            state: States::ActiveSendContactHeader,
            can_tls,
            my_node_id: node_id,
            last_used_transfer_id: 0,
            my_contact_header: None,
            peer_contact_header: None,
            my_sess_init: None,
            peer_sess_init: None,
            terminating: false,
        }
    }
    pub fn new_passive(node_id: String, can_tls: bool) -> Self {
        StateMachine {
            state: States::PassiveWaitContactHeader,
            can_tls,
            my_node_id: node_id,
            last_used_transfer_id: 0,
            my_contact_header: None,
            peer_contact_header: None,
            my_sess_init: None,
            peer_sess_init: None,
            terminating: false,
        }
    }

    pub async fn send_message(
        &mut self,
        writer: &mut FramedWrite<WriteHalf<Pin<Box<dyn AsyncReadWrite>>>, Codec>,
    ) -> Result<(), std::io::Error> {
        match &mut self.state {
            States::ActiveSendContactHeader | States::PassiveSendContactHeader => {
                let ch = ContactHeader::new(self.can_tls);
                self.my_contact_header = Some(ch.clone());
                writer.send(Messages::ContactHeader(ch)).await?;
            }
            States::ActiveSendSessInit | States::PassiveSendSessInit => {
                let si = SessInit::new(self.my_node_id.clone());
                self.my_sess_init = Some(si.clone());
                writer.send(Messages::SessInit(si)).await?;
            }
            States::SendXferAck(xfer_ack) | States::SendXferSegmentsAndAck(_, xfer_ack) => {
                writer.send(Messages::XferAck(xfer_ack.clone())).await?;
            }
            States::SendSessTerm(r) => {
                let st = SessTerm::new(r.unwrap_or(ReasonCode::Unkown), self.terminating);
                writer.send(Messages::SessTerm(st)).await?;
            }
            States::SendXferSegments(tt) => {
                let mru = self.peer_sess_init.as_ref().unwrap().segment_mru;
                let end_pos = min(tt.pos + mru as usize, tt.transfer.data.len());
                if tt.pos == tt.transfer.data.len() {
                    warn!(
                        "We should not try to send a transfer if we already sent all data. We just dont do anything"
                    );
                    return Ok(());
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

                // The following is some magic to ensure we are actually cancelation safe (so send_message can be used in select!)
                // We first feed the data to the writer. According to https://users.rust-lang.org/t/is-tokio-codec-framed-cancel-safe/86408/14
                // this is cancelation safe. So if this future is canceled the message has either been appended to the buffer
                // (and we where at flush below) or it has not yet been appended to the buffer.
                // Only after we have appended to the buffer (and done so successfully) are we allowed to increase our transfer tracking position.
                // The call to flush at the end is just so the data is actually out. It should not hurt if it does not happen as the buffer should
                // be flushed in the background anyway
                writer.feed(Messages::XferSegment(xfer_seg)).await?;
                tt.pos = end_pos;
                writer.flush().await?;
            }
            States::SendKeepalive(_) => {
                let ka = Keepalive::new();
                writer.send(Messages::Keepalive(ka)).await?;
            }
            States::SendMsgReject(r, t) => {
                let mr = MsgReject::new(*r, *t);
                writer.send(Messages::MsgReject(mr)).await?;
            }
            _ => {
                panic!(
                    "Tried to send a message while we should be receiving. State: {:?}",
                    self.state
                );
            }
        }
        self.send_complete();
        Ok(())
    }

    pub fn decode_message(
        &mut self,
        message: Result<Messages, messages::Errors>,
    ) -> Result<Messages, Errors> {
        match self.state {
            States::PassiveWaitContactHeader | States::ActiveWaitContactHeader => {
                let ch = match &message {
                    Ok(Messages::ContactHeader(ch)) => ch,
                    Err(messages::Errors::InvalidHeader) => return Err(Errors::DoesNotSpeakTCPCL),
                    _ => panic!("no idea, {message:?}"),
                };
                self.peer_contact_header = Some(ch.clone());
                if self.state == States::PassiveWaitContactHeader {
                    self.state = States::PassiveSendContactHeader;
                } else {
                    self.state = States::ActiveSendSessInit;
                }
            }
            States::ActiveWaitSessInit
            | States::PassiveWaitSessInit
            | States::WaitSessTerm
            | States::SessionEstablished
            | States::SendXferSegments(_)
            | States::SendXferSegmentsAndAck(_, _)
            | States::SendKeepalive(_) => {
                if let Err(messages::Errors::InvalidMessageType(message_type_num)) = message {
                    self.state = States::SendMsgReject(
                        msg_reject::ReasonCode::MessageTypeUnkown,
                        message_type_num,
                    );
                    return message.map_err(std::convert::Into::into);
                }
                match &message {
                    Ok(Messages::SessInit(si)) if self.state == States::ActiveWaitSessInit => {
                        self.peer_sess_init = Some(si.clone());
                        self.state = States::SessionEstablished;
                    }
                    Ok(Messages::SessInit(si)) if self.state == States::PassiveWaitSessInit => {
                        self.peer_sess_init = Some(si.clone());
                        self.state = States::PassiveSendSessInit;
                    }
                    Ok(Messages::SessTerm(_)) if self.state == States::WaitSessTerm => {
                        self.state = States::ConnectionClose;
                    }
                    Ok(Messages::SessTerm(st))
                        if self.state == States::SessionEstablished
                            || matches!(self.state, States::SendXferSegments(_))
                            || matches!(self.state, States::SendXferSegmentsAndAck(_, _)) =>
                    {
                        self.state = States::SendSessTerm(Some(st.reason));
                        self.terminating = true;
                    }
                    Ok(Messages::XferSegment(_))
                        if self.state == States::SessionEstablished
                            || matches!(self.state, States::SendXferSegments(_))
                            || matches!(self.state, States::SendXferSegmentsAndAck(_, _)) => {}
                    Ok(Messages::XferAck(xa)) => match &mut self.state {
                        States::SendXferSegments(tt) | States::SendXferSegmentsAndAck(tt, _) => {
                            assert!(
                                tt.transfer.id == xa.transfer_id,
                                "Remote send ack for transfer {}, but we are currently sending {}",
                                xa.transfer_id,
                                tt.transfer.id
                            );
                            tt.pos_acked = xa.acknowleged_length as usize;
                            if tt.pos_acked > tt.pos {
                                error!(
                                    "Peer acked a transfer further than we have send it (current send potition {}, current acked position {}).",
                                    tt.pos, tt.pos_acked
                                );
                                return Err(Errors::MessageError(
                                    messages::Errors::InvalidACKValue,
                                ));
                            }
                            if tt.pos_acked == tt.transfer.data.len() {
                                info!("Transfer {} finished (sent and acked)", tt.transfer.id);
                                let state = mem::replace(&mut self.state, States::ShouldNeverExist);
                                match state {
                                    States::SendXferSegments(_) => {
                                        self.state = States::SessionEstablished;
                                    }
                                    States::SendXferSegmentsAndAck(_, ack) => {
                                        self.state = States::SendXferAck(ack);
                                    }
                                    _ => panic!("Invalid state {state:?}"),
                                }
                            }
                        }
                        _ => {
                            warn!(
                                "Received inappropriate message type {:?} while in state {:?}",
                                message, self.state
                            );
                            self.state = States::SendMsgReject(
                                msg_reject::ReasonCode::MessageUnexpected,
                                MessageType::XferAck.into(),
                            );
                            return Err(Errors::MessageTypeInappropriate(MessageType::XferAck));
                        }
                    },
                    Ok(Messages::Keepalive(_) | Messages::MsgReject(_)) | Err(_) => {}
                    Ok(m) => {
                        warn!(
                            "Received inappropriate message type {:?} while in state {:?}",
                            m, self.state
                        );
                        self.state = States::SendMsgReject(
                            msg_reject::ReasonCode::MessageUnexpected,
                            m.get_message_type().into(),
                        );
                        return Err(Errors::MessageTypeInappropriate(m.get_message_type()));
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
        message.map_err(std::convert::Into::into)
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
                self.state = States::SessionEstablished;
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
            States::SendSessTerm(_) if self.terminating => {
                self.state = States::ConnectionClose;
            }
            States::SendSessTerm(_) if !self.terminating => {
                self.terminating = true;
                self.state = States::WaitSessTerm;
            }
            States::SendMsgReject(_, _) => {
                self.terminating = true;
                self.state = States::ConnectionClose;
            }
            _ => {
                panic!("{state:?} is not a valid state to complete sending");
            }
        }
    }

    pub fn send_transfer(&mut self, data: Arc<Vec<u8>>) -> Result<(), TransferSendErrors> {
        if self.peer_sess_init.as_ref().unwrap().transfer_mru < data.len() as u64 {
            return Err(TransferSendErrors::BundleTooLarge {
                max_size: self.peer_sess_init.as_ref().unwrap().transfer_mru,
            });
        }
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
        Ok(())
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
        assert!(
            self.is_established(),
            "Attempted to close a non-established connection"
        );
        self.state = States::SendSessTerm(reason);
    }

    pub fn connection_closing(&self) -> bool {
        matches!(
            self.state,
            States::SendSessTerm(_) | States::WaitSessTerm | States::ConnectionClose
        )
    }

    pub fn get_peer_node_id(&self) -> String {
        assert!(
            self.is_established(),
            "Attempted to get the peer node-id on a non-established connection"
        );
        self.peer_sess_init.as_ref().unwrap().node_id.clone()
    }

    pub fn get_peer_mru(&self) -> u64 {
        assert!(
            self.is_established(),
            "Attempted to get the peer mru on a non-established connection"
        );
        self.peer_sess_init.as_ref().unwrap().transfer_mru
    }

    pub fn get_keepalive_interval(&self) -> Option<u16> {
        assert!(
            self.is_established(),
            "Attempted to get the keepalive interval on a non-established connection"
        );
        let my_keepalive = self.my_sess_init.as_ref().unwrap().keepalive_interval;
        let peer_keepalive = self.peer_sess_init.as_ref().unwrap().keepalive_interval;
        let keepalive = min(my_keepalive, peer_keepalive);
        match keepalive {
            0 => None,
            x => Some(x),
        }
    }

    pub fn contact_header_done(&self) -> bool {
        !matches!(
            self.state,
            States::ActiveSendContactHeader
                | States::ActiveWaitContactHeader
                | States::PassiveSendContactHeader
                | States::PassiveWaitContactHeader
        )
    }

    pub fn should_use_tls(&self) -> bool {
        assert!(
            !(self.my_contact_header.is_none() || self.peer_contact_header.is_none()),
            "May not access tls info if we don't have a contact header yet"
        );
        self.my_contact_header.as_ref().unwrap().can_tls()
            && self.peer_contact_header.as_ref().unwrap().can_tls()
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
