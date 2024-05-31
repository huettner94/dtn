use std::collections::HashMap;
use std::fmt::Display;
use std::path::PathBuf;
use std::pin::Pin;

use super::contentaddressableblob::ContentAddressableBlobStore;
use super::keyvalue::KeyValueStore;

use actix::prelude::*;
use bytes::Bytes;

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum StoreType {
    ContentAddressableBlob,
    KeyValue,
}

impl StoreType {
    pub fn as_bytes(&self) -> &[u8] {
        match self {
            StoreType::ContentAddressableBlob => b"ContentAddressableBlob",
            StoreType::KeyValue => b"KeyValue",
        }
    }
}

impl Display for StoreType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            StoreType::ContentAddressableBlob => "ContentAddressableBlob",
            StoreType::KeyValue => "KeyValue",
        })
    }
}

#[derive(Debug)]
pub struct StoreError {
    rocks_error: rocksdb::Error,
}

impl From<rocksdb::Error> for StoreError {
    fn from(value: rocksdb::Error) -> Self {
        StoreError { rocks_error: value }
    }
}

#[derive(Debug)]
pub enum GetOrCreateError {
    StoreTypeMissmatch(String, String),
    StoreError(StoreError),
}

impl From<rocksdb::Error> for GetOrCreateError {
    fn from(value: rocksdb::Error) -> Self {
        Self::StoreError(value.into())
    }
}

#[derive(Message)]
#[rtype(result = "Result<Addr<KeyValueStore>, GetOrCreateError>")]
pub struct GetOrCreateKeyValueStore {
    pub name: String,
}

#[derive(Message)]
#[rtype(result = "Result<Addr<ContentAddressableBlobStore>, GetOrCreateError>")]
pub struct GetOrCreateContentAddressableBlobStore {
    pub name: String,
    pub path: PathBuf,
}

#[derive(Message)]
#[rtype(result = "Result<Option<String>, StoreError>")]
pub struct Get {
    pub key: Vec<String>,
}

#[derive(Message)]
#[rtype(result = "Result<(), StoreError>")]
pub struct Set {
    pub key: Vec<String>,
    pub value: String,
}

#[derive(Message)]
#[rtype(result = "Result<(), StoreError>")]
pub struct MultiSet {
    pub data: HashMap<Vec<String>, String>,
}

#[derive(Message)]
#[rtype(result = "Result<(), StoreError>")]
pub struct Delete {
    pub key: Vec<String>,
}

#[derive(Message)]
#[rtype(result = "Result<HashMap<String, String>, StoreError>")]
pub struct List {
    pub prefix: Vec<String>,
}

#[derive(Debug)]
pub struct PutBlobReadError {
    pub msg: String,
}

pub enum PutBlobError {
    StoreError(StoreError),
    PutBlobReadError(PutBlobReadError),
    IoError(std::io::Error),
}

impl From<std::io::Error> for PutBlobError {
    fn from(value: std::io::Error) -> Self {
        Self::IoError(value)
    }
}

impl From<PutBlobReadError> for PutBlobError {
    fn from(value: PutBlobReadError) -> Self {
        Self::PutBlobReadError(value)
    }
}

#[derive(Debug)]
pub struct BlobInfo {
    pub md5sum: String,
    pub sha256sum: String,
}

#[derive(Message)]
#[rtype(result = "Result<BlobInfo, PutBlobError>")]
pub struct PutBlob {
    pub data: Pin<Box<dyn Stream<Item = Result<Bytes, PutBlobReadError>> + Send>>,
}
