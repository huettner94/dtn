use actix::prelude::*;
use time::OffsetDateTime;

use crate::stores::messages::StoreError;

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

pub enum PutObjectError {
    S3Error(S3Error),
    BucketNotFound,
}

impl From<StoreError> for PutObjectError {
    fn from(value: StoreError) -> Self {
        Self::S3Error(value.into())
    }
}

#[derive(Message)]
#[rtype(result = "Result<Object, PutObjectError>")]
pub struct PutObject {
    pub bucket: String,
    pub key: String,
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
