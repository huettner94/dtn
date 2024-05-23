use actix::prelude::*;
use rocksdb::DB;
use std::{collections::HashMap, sync::Arc};

use super::messages::{Delete, Get, List, Set, StoreError};

#[derive(Debug)]
pub struct KeyValueStore {
    name: String,
    db: Arc<DB>,
}

impl KeyValueStore {
    pub fn new(name: String, db: Arc<DB>) -> Self {
        KeyValueStore { name, db }
    }

    fn get_path(&self, keys: Vec<String>) -> String {
        format!("\0store\0{}\0{}", self.name, keys.join("\0"))
    }
}

impl Actor for KeyValueStore {
    type Context = Context<Self>;
}

impl Handler<Get> for KeyValueStore {
    type Result = Result<Option<String>, StoreError>;

    fn handle(&mut self, msg: Get, _ctx: &mut Self::Context) -> Self::Result {
        let Get { key } = msg;
        Ok(self
            .db
            .get(&self.get_path(key))?
            .map(|e| String::from_utf8(e).unwrap()))
    }
}

impl Handler<Set> for KeyValueStore {
    type Result = Result<(), StoreError>;

    fn handle(&mut self, msg: Set, _ctx: &mut Self::Context) -> Self::Result {
        let Set { key, value } = msg;
        Ok(self.db.put(&self.get_path(key), value)?)
    }
}

impl Handler<Delete> for KeyValueStore {
    type Result = Result<(), StoreError>;

    fn handle(&mut self, msg: Delete, _ctx: &mut Self::Context) -> Self::Result {
        let Delete { key } = msg;
        Ok(self.db.delete(&self.get_path(key))?)
    }
}

impl Handler<List> for KeyValueStore {
    type Result = Result<HashMap<String, String>, StoreError>;

    fn handle(&mut self, msg: List, _ctx: &mut Self::Context) -> Self::Result {
        let List { prefix } = msg;
        let path = self.get_path(prefix);
        let path_bytes = path.as_bytes();
        let mut options = rocksdb::ReadOptions::default();
        options.set_iterate_range(rocksdb::PrefixRange(path_bytes));
        self.db
            .iterator_opt(
                rocksdb::IteratorMode::From(&path_bytes, rocksdb::Direction::Forward),
                options,
            )
            .try_fold(HashMap::new(), |mut map, e| {
                let (key, value) = e?;
                let keystring =
                    String::from_utf8(key.iter().cloned().skip(path_bytes.len()).collect())
                        .unwrap();
                map.insert(keystring, String::from_utf8(value.to_vec()).unwrap());
                Ok(map)
            })
    }
}
