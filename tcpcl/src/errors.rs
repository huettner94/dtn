use openssl::error::ErrorStack;

use crate::v4::messages::{self, MessageType};

#[derive(Debug)]
pub enum Errors {
    MessageTypeInappropriate(MessageType),
    RemoteRejected,
    DoesNotSpeakTCPCL,
    TLSNameMissmatch(String),
    MessageError(messages::Errors),
}

#[derive(Debug)]
pub enum ErrorType {
    IOError(std::io::Error),
    SSLError(openssl::ssl::Error),
    TCPCLError(Errors),
    DnsError,
}

impl From<std::io::Error> for ErrorType {
    fn from(e: std::io::Error) -> Self {
        ErrorType::IOError(e)
    }
}

impl From<ErrorStack> for ErrorType {
    fn from(e: ErrorStack) -> Self {
        ErrorType::SSLError(e.into())
    }
}

impl From<openssl::ssl::Error> for ErrorType {
    fn from(e: openssl::ssl::Error) -> Self {
        ErrorType::SSLError(e)
    }
}

impl From<Errors> for ErrorType {
    fn from(e: Errors) -> Self {
        ErrorType::TCPCLError(e)
    }
}

impl From<messages::Errors> for ErrorType {
    fn from(value: messages::Errors) -> Self {
        ErrorType::TCPCLError(Errors::MessageError(value))
    }
}

impl From<messages::Errors> for Errors {
    fn from(value: messages::Errors) -> Self {
        Errors::MessageError(value)
    }
}

#[derive(Debug)]
pub enum TransferSendErrors {
    BundleTooLarge { max_size: u64 },
}
