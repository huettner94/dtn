use std::pin::Pin;

use actix::prelude::*;
use time::OffsetDateTime;

use crate::stores::messages::{DeleteBlobError, GetBlobError, PutBlobError, StoreError};

#[derive(Debug)]
pub struct S3Error {
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
            PutBlobError::StoreError(e) => e.into(),
            PutBlobError::BlobReadError(e) => Self::ReadDataError(ReadDataError { msg: e.msg }),
            PutBlobError::IoError(e) => Self::ReadDataError(ReadDataError { msg: e.to_string() }),
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
            GetBlobError::StoreError(e) => e.into(),
            GetBlobError::BlobReadError(e) => Self::ReadDataError(ReadDataError { msg: e.msg }),
            GetBlobError::IoError(e) => Self::ReadDataError(ReadDataError { msg: e.to_string() }),
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
