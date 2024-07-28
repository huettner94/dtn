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
use std::{collections::HashMap, sync::Arc};

use super::messages::{Delete, Get, List, MultiDelete, MultiSet, Set, StoreError};

pub struct KeyValueStore {
    name: String,
    db: Arc<TransactionDB>,
}

impl std::fmt::Debug for KeyValueStore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("KeyValueStore")
            .field("name", &self.name)
            .finish()
    }
}

impl KeyValueStore {
    pub fn new(name: String, db: Arc<TransactionDB>) -> Self {
        KeyValueStore { name, db }
    }

    fn get_path(&self, keys: Vec<String>) -> String {
        format!("\0store\0{}\0{}", self.name, keys.join("\0"))
    }

    fn iter_range(&self, key: &[u8]) -> rocksdb::DBIteratorWithThreadMode<'_, TransactionDB> {
        let mut options = rocksdb::ReadOptions::default();
        options.set_iterate_range(rocksdb::PrefixRange(key));
        self.db.iterator_opt(
            rocksdb::IteratorMode::From(key, rocksdb::Direction::Forward),
            options,
        )
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

impl Handler<MultiSet> for KeyValueStore {
    type Result = Result<(), StoreError>;

    fn handle(&mut self, msg: MultiSet, _ctx: &mut Self::Context) -> Self::Result {
        let MultiSet { mut data } = msg;
        let txn = self.db.transaction();
        for (key, value) in data.drain() {
            txn.put(&self.get_path(key), value)?;
        }
        txn.commit()?;
        Ok(())
    }
}

impl Handler<Delete> for KeyValueStore {
    type Result = Result<(), StoreError>;

    fn handle(&mut self, msg: Delete, _ctx: &mut Self::Context) -> Self::Result {
        let Delete { key } = msg;
        Ok(self.db.delete(&self.get_path(key))?)
    }
}

impl Handler<MultiDelete> for KeyValueStore {
    type Result = Result<(), StoreError>;

    fn handle(&mut self, msg: MultiDelete, _ctx: &mut Self::Context) -> Self::Result {
        let MultiDelete { mut data } = msg;
        let txn = self.db.transaction();
        for key in data.drain(..) {
            let path = self.get_path(key);
            let path_bytes = path.as_bytes();
            for found in self.iter_range(path_bytes) {
                txn.delete(found?.0)?
            }
        }
        txn.commit()?;
        Ok(())
    }
}

impl Handler<List> for KeyValueStore {
    type Result = Result<HashMap<String, String>, StoreError>;

    fn handle(&mut self, msg: List, _ctx: &mut Self::Context) -> Self::Result {
        let List { prefix } = msg;
        let path = self.get_path(prefix);
        let path_bytes = path.as_bytes();
        self.iter_range(path_bytes)
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
