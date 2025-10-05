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

use std::pin::Pin;

use actix::prelude::*;
use time::OffsetDateTime;

use crate::stores::messages::{DeleteBlobError, GetBlobError, PutBlobError, StoreError};

#[derive(Debug)]
pub struct S3Error {
    #[allow(dead_code)] // Only used for debug output
    store_error: StoreError,
}

impl From<StoreError> for S3Error {
    fn from(value: StoreError) -> Self {
        S3Error { store_error: value }
    }
}

#[derive(Debug)]
pub struct Object {
    pub key: String,
    pub md5sum: String,
    pub sha256sum: String,
    pub last_modified: OffsetDateTime,
    pub size: u64,
}

#[derive(Message)]
#[rtype(result = "Result<Vec<String>, S3Error>")]
pub struct ListBuckets {}

pub enum CreateBucketError {
    S3Error(S3Error),
    BucketAlreadyExists,
}

impl From<StoreError> for CreateBucketError {
    fn from(value: StoreError) -> Self {
        Self::S3Error(value.into())
    }
}

#[derive(Message)]
#[rtype(result = "Result<String, CreateBucketError>")]
pub struct CreateBucket {
    pub name: String,
}

#[derive(Message)]
#[rtype(result = "Result<Option<()>, S3Error>")]
pub struct HeadBucket {
    pub name: String,
}

#[derive(Debug)]
pub struct ReadDataError {
    pub msg: String,
}

pub enum PutObjectError {
    S3Error(S3Error),
    BucketNotFound,
    ReadDataError(ReadDataError),
}

impl From<StoreError> for PutObjectError {
    fn from(value: StoreError) -> Self {
        Self::S3Error(value.into())
    }
}

impl From<PutBlobError> for PutObjectError {
    fn from(value: PutBlobError) -> Self {
        match value {
            PutBlobError::Store(e) => e.into(),
            PutBlobError::BlobRead(e) => Self::ReadDataError(ReadDataError { msg: e.msg }),
            PutBlobError::Io(e) => Self::ReadDataError(ReadDataError { msg: e.to_string() }),
        }
    }
}

#[derive(Message)]
#[rtype(result = "Result<Object, PutObjectError>")]
pub struct PutObject {
    pub bucket: String,
    pub key: String,
    pub data: Pin<Box<dyn Stream<Item = Result<bytes::Bytes, ReadDataError>> + Send>>,
}

pub enum ListObjectError {
    S3Error(S3Error),
    BucketNotFound,
}

impl From<StoreError> for ListObjectError {
    fn from(value: StoreError) -> Self {
        Self::S3Error(value.into())
    }
}

#[derive(Message)]
#[rtype(result = "Result<Vec<Object>, ListObjectError>")]
pub struct ListObject {
    pub bucket: String,
    pub prefix: String,
}

pub enum HeadObjectError {
    S3Error(S3Error),
    BucketNotFound,
    ObjectNotFound,
}

impl From<StoreError> for HeadObjectError {
    fn from(value: StoreError) -> Self {
        Self::S3Error(value.into())
    }
}

#[derive(Message)]
#[rtype(result = "Result<Object, HeadObjectError>")]
pub struct HeadObject {
    pub bucket: String,
    pub key: String,
}

pub enum GetObjectError {
    S3Error(S3Error),
    BucketNotFound,
    ObjectNotFound,
    ReadDataError(ReadDataError),
}

impl From<StoreError> for GetObjectError {
    fn from(value: StoreError) -> Self {
        Self::S3Error(value.into())
    }
}

impl From<GetBlobError> for GetObjectError {
    fn from(value: GetBlobError) -> Self {
        match value {
            GetBlobError::Store(e) => e.into(),
            GetBlobError::BlobRead(e) => Self::ReadDataError(ReadDataError { msg: e.msg }),
            GetBlobError::Io(e) => Self::ReadDataError(ReadDataError { msg: e.to_string() }),
            GetBlobError::BlobDoesNotExist => GetObjectError::ObjectNotFound,
        }
    }
}

pub struct GetObjectResult {
    pub metadata: Object,
    pub data: Pin<Box<dyn Stream<Item = Result<bytes::Bytes, ReadDataError>> + Send + Sync>>,
}

#[derive(Message)]
#[rtype(result = "Result<GetObjectResult, GetObjectError>")]
pub struct GetObject {
    pub bucket: String,
    pub key: String,
}

pub enum DeleteObjectError {
    S3Error(S3Error),
    BucketNotFound,
    ObjectNotFound,
    ReadDataError(ReadDataError),
}

impl From<StoreError> for DeleteObjectError {
    fn from(value: StoreError) -> Self {
        Self::S3Error(value.into())
    }
}

impl From<DeleteBlobError> for DeleteObjectError {
    fn from(value: DeleteBlobError) -> Self {
        match value {
            DeleteBlobError::StoreError(e) => e.into(),
            DeleteBlobError::IoError(e) => {
                Self::ReadDataError(ReadDataError { msg: e.to_string() })
            }
            DeleteBlobError::BlobDoesNotExist => DeleteObjectError::ObjectNotFound,
        }
    }
}

#[derive(Message)]
#[rtype(result = "Result<(), DeleteObjectError>")]
pub struct DeleteObject {
    pub bucket: String,
    pub key: String,
}
