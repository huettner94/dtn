use tonic::codegen::http::uri::InvalidUri;

#[derive(Debug)]
pub enum Error {
    InvalidUrl,
    TransportError(tonic::transport::Error),
    GrpcError(tonic::Status),
}

impl From<InvalidUri> for Error {
    fn from(_: InvalidUri) -> Self {
        Error::InvalidUrl
    }
}

impl From<tonic::transport::Error> for Error {
    fn from(err: tonic::transport::Error) -> Self {
        Error::TransportError(err)
    }
}

impl From<tonic::Status> for Error {
    fn from(err: tonic::Status) -> Self {
        Error::GrpcError(err)
    }
}
