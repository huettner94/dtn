use super::{
    contact_header::ContactHeader, errors::Errors, reader::Reader, transform::Transform, Messages,
};

enum States {
    ACTIVE_SEND_CONTACT_HEADER,
    PASSIVE_WAIT_CONTACT_HEADER,
}

pub struct StateMachine {
    state: States,
}

impl StateMachine {
    pub fn new_active() -> Self {
        StateMachine {
            state: States::ACTIVE_SEND_CONTACT_HEADER,
        }
    }
    pub fn new_passive() -> Self {
        StateMachine {
            state: States::PASSIVE_WAIT_CONTACT_HEADER,
        }
    }

    pub fn decode_message(&self, reader: &mut Reader) -> Result<Messages, Errors> {
        let out = self.decode_message_inner(reader);
        if out.is_ok() {
            reader.consume();
        } else {
            reader.reset_read();
        }
        return out;
    }

    pub fn decode_message_inner(&self, reader: &mut Reader) -> Result<Messages, Errors> {
        match self.state {
            States::PASSIVE_WAIT_CONTACT_HEADER => {
                let ch = ContactHeader::read(reader)?;
                Ok(Messages::ContactHeader(ch))
            }
            _ => {
                panic!("Tried to decode a message while we should be sending")
            }
        }
    }
}
