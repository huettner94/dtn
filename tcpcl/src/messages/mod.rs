use self::contact_header::ContactHeader;

pub mod contact_header;
pub mod errors;
pub mod reader;
pub mod statemachine;
pub mod transform;

#[derive(Debug)]
pub enum Messages {
    ContactHeader(ContactHeader),
}
