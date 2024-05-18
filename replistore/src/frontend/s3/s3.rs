use actix::prelude::*;
use log::error;

use crate::stores::{keyvalue::KeyValueStore, storeowner::StoreOwner};

use super::messages::{CreateBucket, CreateBucketError, HeadBucket, ListBuckets, S3Error};

#[derive(Debug)]
pub struct S3 {
    store_owner: Addr<StoreOwner>,
    s3_kv_store: Option<Addr<KeyValueStore>>,
}

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
                    prefix: "buckets/".to_string(),
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
        let bucket_path = format!("buckets/{}", name);
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
        let bucket_path = format!("buckets/{}", name);
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
