pub mod administrative_record;
pub mod block;
pub mod blockflags;
pub mod bundle;
pub mod bundleflags;
pub mod crc;
pub mod endpoint;
pub mod primaryblock;
pub mod time;

pub trait Validate {
    fn validate(&self) -> bool;
}

#[derive(Debug)]
pub enum SerializationError {
    SerializationError(serde_cbor::Error),
    ConversionError,
}

impl From<serde_cbor::Error> for SerializationError {
    fn from(error: serde_cbor::Error) -> Self {
        SerializationError::SerializationError(error)
    }
}

#[derive(Debug)]
pub enum FragmentationError {
    SerializationError(SerializationError),
    CanNotFragmentThatSmall,
    MustNotFragment,
    BundleInvalid,
}

impl From<SerializationError> for FragmentationError {
    fn from(e: SerializationError) -> Self {
        FragmentationError::SerializationError(e)
    }
}

impl From<serde_cbor::Error> for FragmentationError {
    fn from(error: serde_cbor::Error) -> Self {
        FragmentationError::SerializationError(SerializationError::SerializationError(error))
    }
}
