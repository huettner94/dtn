use tokio::io::Interest;

use crate::errors::Errors;

use super::{contact_header::ContactHeader, reader::Reader, transform::Transform, Messages};

#[derive(Debug, PartialEq, Eq)]
pub enum States {
    // Handshake Part 1
    ActiveSendContactHeader,
    PassiveWaitContactHeader,
    // Handshake Part 2
    ActiveWaitContactHeader,
    PassiveSendContactHeader,
    // Final
    ConnectionClose,
}

#[derive(Debug)]
pub struct StateMachine {
    pub state: States,
    my_contact_header: Option<ContactHeader>,
    peer_contact_header: Option<ContactHeader>,
}

impl StateMachine {
    pub fn new_active() -> Self {
        StateMachine {
            state: States::ActiveSendContactHeader,
            my_contact_header: None,
            peer_contact_header: None,
        }
    }
    pub fn new_passive() -> Self {
        StateMachine {
            state: States::PassiveWaitContactHeader,
            my_contact_header: None,
            peer_contact_header: None,
        }
    }

    pub fn send_message(&mut self, writer: &mut Vec<u8>) {
        match self.state {
            States::ActiveSendContactHeader | States::PassiveSendContactHeader => {
                let ch = ContactHeader::new();
                self.my_contact_header = Some(ch.clone());
                ch.write(writer);
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

    pub fn decode_message_inner(&mut self, reader: &mut Reader) -> Result<Messages, Errors> {
        match self.state {
            States::PassiveWaitContactHeader | States::ActiveWaitContactHeader => {
                let ch = ContactHeader::read(reader)?;
                self.peer_contact_header = Some(ch.clone());
                Ok(Messages::ContactHeader(ch))
            }
            _ => {
                panic!("Tried to decode a message while we should be sending")
            }
        }
    }

    pub fn get_interests(&self) -> Interest {
        match self.state {
            States::ActiveSendContactHeader | States::PassiveSendContactHeader => {
                Interest::WRITABLE
            }
            States::PassiveWaitContactHeader | States::ActiveWaitContactHeader => {
                Interest::READABLE
            }
            States::ConnectionClose => {
                panic!("Tried to continue after connection should be closed")
            }
        }
    }

    pub fn state_complete(&mut self) {
        match self.state {
            States::ActiveSendContactHeader => self.state = States::ActiveWaitContactHeader,
            States::PassiveWaitContactHeader => self.state = States::PassiveSendContactHeader,
            States::ActiveWaitContactHeader => self.state = States::ConnectionClose,
            States::PassiveSendContactHeader => self.state = States::ConnectionClose,
            States::ConnectionClose => {
                panic!("Tried to continue after connection should be closed")
            }
        }
    }

    pub fn should_close(&self) -> bool {
        return self.state == States::ConnectionClose;
    }
}
