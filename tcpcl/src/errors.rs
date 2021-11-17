#[derive(Debug)]
pub enum Errors {
    MessageTooShort,
    InvalidHeader,
    NodeIdInvalid,
    UnkownCriticalSessionExtension(u16),
    UnkownCriticalTransferExtension(u16),
    UnkownMessageType,
    MessageTypeInappropriate,
    RemoteRejected,
    TLSNameMissmatch(String),
}

#[derive(Debug)]
pub enum ErrorType {
    IOError(std::io::Error),
    TCPCLError(Errors),
}

impl From<std::io::Error> for ErrorType {
    fn from(e: std::io::Error) -> Self {
        ErrorType::IOError(e)
    }
}

impl From<Errors> for ErrorType {
    fn from(e: Errors) -> Self {
        ErrorType::TCPCLError(e)
    }
}
