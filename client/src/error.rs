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

use std::fmt::Display;

use tonic::codegen::http::uri::InvalidUri;

#[derive(Debug)]
pub enum Error {
    InvalidUrl,
    TransportError(tonic::transport::Error),
    GrpcError(tonic::Status),
    NoMessage,
}

impl Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::InvalidUrl => f.write_str("Invalid URL"),
            Error::TransportError(error) => f.write_fmt(format_args!(
                "Error when communicating with dtrd: {error}"
            )),
            Error::GrpcError(status) => f.write_fmt(format_args!(
                "Error when communicating with dtrd: {status}"
            )),
            Error::NoMessage => f.write_str("No Message to be received"),
        }
    }
}

impl std::error::Error for Error {}

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
