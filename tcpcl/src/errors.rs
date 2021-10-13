#[derive(Debug)]
pub enum Errors {
    MessageTooShort,
    InvalidHeader,
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
