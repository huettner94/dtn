use openssl::error::ErrorStack;

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
    SSLError(openssl::ssl::Error),
    TCPCLError(Errors),
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

#[derive(Debug)]
pub enum TransferSendErrors {
    BundleTooLarge { max_size: u64 },
}
