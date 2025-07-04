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

use actix::prelude::*;
use rocksdb::TransactionDB;
use std::{collections::HashMap, path::PathBuf, sync::Arc};

use crate::replication::Replicator;

use super::{
    contentaddressableblob::ContentAddressableBlobStore,
    keyvalue::KeyValueStore,
    messages::{
        GetOrCreateContentAddressableBlobStore, GetOrCreateError, GetOrCreateKeyValueStore,
        StoreType,
    },
};

pub struct StoreOwner {
    db: Arc<TransactionDB>,
    replicator: Addr<Replicator>,
    kv_stores: HashMap<String, Addr<KeyValueStore>>,
    blob_stores: HashMap<String, Addr<ContentAddressableBlobStore>>,
}

impl std::fmt::Debug for StoreOwner {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StoreOwner")
            .field("kv_stores", &self.kv_stores)
            .field("blob_stores", &self.blob_stores)
            .finish()
    }
}

impl StoreOwner {
    pub fn new(db_path: PathBuf, replicator: Addr<Replicator>) -> Result<Self, rocksdb::Error> {
        let db = TransactionDB::open_default(db_path)?;
        Ok(StoreOwner {
            db: Arc::new(db),
            replicator,
            kv_stores: HashMap::new(),
            blob_stores: HashMap::new(),
        })
    }

    fn check_or_create_store_type(
        &self,
        name: &str,
        store_type: StoreType,
    ) -> Result<Option<()>, GetOrCreateError> {
        let type_key = format!("\0type\0{}", name);
        match self.db.get(&type_key)? {
            Some(entry) => {
                if entry == store_type.as_bytes() {
                    Ok(Some(()))
                } else {
                    Err(GetOrCreateError::StoreTypeMissmatch(
                        store_type.to_string(),
                        String::from_utf8(entry).unwrap(),
                    ))
                }
            }
            None => {
                self.db.put(&type_key, store_type.as_bytes())?;
                Ok(Some(()))
            }
        }
    }
}

impl Actor for StoreOwner {
    type Context = Context<Self>;
}

impl Handler<GetOrCreateKeyValueStore> for StoreOwner {
    type Result = Result<Addr<KeyValueStore>, GetOrCreateError>;

    fn handle(&mut self, msg: GetOrCreateKeyValueStore, _ctx: &mut Context<Self>) -> Self::Result {
        let GetOrCreateKeyValueStore { name } = msg;
        self.check_or_create_store_type(&name, StoreType::KeyValue)?;
        match self.kv_stores.get(&name) {
            Some(addr) => Ok(addr.clone()),
            None => {
                let kv_store =
                    KeyValueStore::new(name.clone(), self.db.clone(), self.replicator.clone())
                        .start();
                self.kv_stores.insert(name, kv_store.clone());
                Ok(kv_store)
            }
        }
    }
}

impl Handler<GetOrCreateContentAddressableBlobStore> for StoreOwner {
    type Result = Result<Addr<ContentAddressableBlobStore>, GetOrCreateError>;

    fn handle(
        &mut self,
        msg: GetOrCreateContentAddressableBlobStore,
        _ctx: &mut Context<Self>,
    ) -> Self::Result {
        let GetOrCreateContentAddressableBlobStore { name, path } = msg;
        self.check_or_create_store_type(&name, StoreType::ContentAddressableBlob)?;
        match self.blob_stores.get(&name) {
            Some(addr) => Ok(addr.clone()),
            None => {
                let blob_store = ContentAddressableBlobStore::new(
                    name.clone(),
                    path,
                    self.db.clone(),
                    self.replicator.clone(),
                )
                .start();
                self.blob_stores.insert(name, blob_store.clone());
                Ok(blob_store)
            }
        }
    }
}
