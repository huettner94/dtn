use std::collections::HashMap;

use actix::prelude::*;
use log::error;
use time::OffsetDateTime;

use crate::stores::{keyvalue::KeyValueStore, storeowner::StoreOwner};

use super::messages::{
    CreateBucket, CreateBucketError, HeadBucket, ListBuckets, ListObject, ListObjectError, Object,
    PutObject, PutObjectError, S3Error,
};

#[derive(Debug)]
pub struct S3 {
    store_owner: Addr<StoreOwner>,
    s3_kv_store: Option<Addr<KeyValueStore>>,
}

/* Kv Structure
 *
 * \0buckets\0<bucket_name>: nil
 * \0objects\0<bucket_name>\0<object_name_path>: nil
 * \0objectmeta\0<bucket_name>\0<object_name_path>\0size: size in bytes
 * \0objectmeta\0<bucket_name>\0<object_name_path>\0last_modified: u64 timestamp
 *
 */

impl S3 {
    pub fn new(store_owner: Addr<StoreOwner>) -> Self {
        S3 {
            store_owner,
            s3_kv_store: None,
        }
    }

    fn store(&self) -> Addr<KeyValueStore> {
        self.s3_kv_store.clone().unwrap()
    }

    fn bucket_path(&self, name: &str) -> Vec<String> {
        vec!["buckets".to_string(), name.to_string()]
    }

    fn object_path(&self, bucket: &str, key: &str) -> Vec<String> {
        vec!["objects".to_string(), bucket.to_string(), key.to_string()]
    }
    fn objectmeta_path(&self, bucket: &str, key: &str, suffix: &str) -> Vec<String> {
        vec![
            "objectmeta".to_string(),
            bucket.to_string(),
            key.to_string(),
            suffix.to_string(),
        ]
    }
}

impl Actor for S3 {
    type Context = Context<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        let store_owner = self.store_owner.clone();
        let fut = async move {
            store_owner
                .send(crate::stores::messages::GetOrCreateKeyValueStore {
                    name: "s3metadata".to_string(),
                })
                .await
        };
        fut.into_actor(self)
            .then(|res, act, ctx| {
                match res.unwrap() {
                    Ok(addr) => act.s3_kv_store = Some(addr),
                    Err(e) => {
                        error!("Error getting keyvalue store {:?}", e);
                        ctx.stop();
                    }
                }
                fut::ready(())
            })
            .wait(ctx)
    }
}

impl Handler<ListBuckets> for S3 {
    type Result = ResponseFuture<Result<Vec<String>, S3Error>>;
    fn handle(&mut self, _msg: ListBuckets, _ctx: &mut Self::Context) -> Self::Result {
        let store = self.store();
        Box::pin(async move {
            let resp = store
                .send(crate::stores::messages::List {
                    prefix: vec!["buckets".to_string(), "".to_string()],
                })
                .await
                .unwrap()?;
            Ok(resp.into_keys().collect())
        })
    }
}

impl Handler<CreateBucket> for S3 {
    type Result = ResponseFuture<Result<String, CreateBucketError>>;
    fn handle(&mut self, msg: CreateBucket, _ctx: &mut Self::Context) -> Self::Result {
        let CreateBucket { name } = msg;
        let store = self.store();
        let bucket_path = self.bucket_path(&name);
        Box::pin(async move {
            let resp = store
                .send(crate::stores::messages::Get {
                    key: bucket_path.clone(),
                })
                .await
                .unwrap()?;
            if resp.is_some() {
                return Err(CreateBucketError::BucketAlreadyExists);
            }
            store
                .send(crate::stores::messages::Set {
                    key: bucket_path,
                    value: String::new(),
                })
                .await
                .unwrap()?;
            Ok(name)
        })
    }
}

impl Handler<HeadBucket> for S3 {
    type Result = ResponseFuture<Result<Option<()>, S3Error>>;

    fn handle(&mut self, msg: HeadBucket, _ctx: &mut Self::Context) -> Self::Result {
        let HeadBucket { name } = msg;
        let store = self.store();
        let bucket_path = self.bucket_path(&name);
        Box::pin(async move {
            let resp = store
                .send(crate::stores::messages::Get {
                    key: bucket_path.clone(),
                })
                .await
                .unwrap()?;
            if resp.is_some() {
                return Ok(Some(()));
            }
            Ok(None)
        })
    }
}

impl Handler<ListObject> for S3 {
    type Result = ResponseFuture<Result<Vec<Object>, ListObjectError>>;

    fn handle(&mut self, msg: ListObject, _ctx: &mut Self::Context) -> Self::Result {
        let ListObject { bucket, prefix } = msg;
        let store = self.store();
        let bucket_path = self.bucket_path(&bucket);
        let object_path = self.object_path(&bucket, &prefix);
        Box::pin(async move {
            let resp = store
                .send(crate::stores::messages::Get {
                    key: bucket_path.clone(),
                })
                .await
                .unwrap()?;
            if resp.is_none() {
                return Err(ListObjectError::BucketNotFound);
            }
            let mut result = Vec::new();
            for obj in store
                .send(crate::stores::messages::List {
                    prefix: object_path,
                })
                .await
                .unwrap()?
                .into_keys()
            {
                let meta = store
                    .send(crate::stores::messages::List {
                        prefix: vec![
                            "objectmeta".to_string(),
                            bucket.clone(),
                            obj.clone(),
                            String::new(),
                        ],
                    })
                    .await
                    .unwrap()?;
                let last_modified = OffsetDateTime::from_unix_timestamp(
                    meta.get("last_modified")
                        .map(|e| e.parse().unwrap_or_default())
                        .unwrap_or_default(),
                )
                .unwrap();
                result.push(Object {
                    key: obj,
                    md5sum: String::new(),
                    sha256sum: String::new(),
                    last_modified,
                });
            }
            Ok(result)
        })
    }
}

impl Handler<PutObject> for S3 {
    type Result = ResponseFuture<Result<Object, PutObjectError>>;

    fn handle(&mut self, msg: PutObject, _ctx: &mut Self::Context) -> Self::Result {
        let PutObject { bucket, key } = msg;
        let store = self.store();
        let bucket_path = self.bucket_path(&bucket);
        let object_path = self.object_path(&bucket, &key);
        let last_modified_path = self.objectmeta_path(&bucket, &key, "last_modified");
        Box::pin(async move {
            let resp = store
                .send(crate::stores::messages::Get {
                    key: bucket_path.clone(),
                })
                .await
                .unwrap()?;
            if resp.is_none() {
                return Err(PutObjectError::BucketNotFound);
            }
            let last_modified = OffsetDateTime::now_utc();
            store
                .send(crate::stores::messages::MultiSet {
                    data: HashMap::from([
                        (object_path.clone(), String::new()),
                        (
                            last_modified_path,
                            last_modified.unix_timestamp().to_string(),
                        ),
                    ]),
                })
                .await
                .unwrap()?;
            Ok(Object {
                key,
                md5sum: String::new(),
                sha256sum: String::new(),
                last_modified,
            })
        })
    }
}
