use actix::prelude::*;
use rocksdb::TransactionDB;
use std::{collections::HashMap, path::PathBuf, sync::Arc};

use super::{
    keyvalue::KeyValueStore,
    messages::{GetOrCreateError, GetOrCreateKeyValueStore, StoreType},
};

pub struct StoreOwner {
    db: Arc<TransactionDB>,
    kv_stores: HashMap<String, Addr<KeyValueStore>>,
}

impl std::fmt::Debug for StoreOwner {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StoreOwner")
            .field("kv_stores", &self.kv_stores)
            .finish()
    }
}

impl StoreOwner {
    pub fn new(db_path: PathBuf) -> Result<Self, rocksdb::Error> {
        let db = TransactionDB::open_default(db_path)?;
        Ok(StoreOwner {
            db: Arc::new(db),
            kv_stores: HashMap::new(),
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
                let kv_store = KeyValueStore::new(name.clone(), self.db.clone()).start();
                self.kv_stores.insert(name, kv_store.clone());
                Ok(kv_store)
            }
        }
    }
}
