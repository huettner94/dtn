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
    CanNotFragmentThatSmall(u64),
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
