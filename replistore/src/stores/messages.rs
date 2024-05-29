use std::collections::HashMap;
use std::fmt::Display;

use super::keyvalue::KeyValueStore;

use actix::prelude::*;

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
