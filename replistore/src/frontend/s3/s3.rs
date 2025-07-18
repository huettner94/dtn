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

use std::collections::HashMap;

use actix::prelude::*;
use futures::{Future, TryStreamExt};
use log::error;
use time::OffsetDateTime;

use crate::{
    replication::{
        messages::{Event, ObjectMeta, ReplicateEvent, BucketEvent},
        Replicator,
    },
    stores::{
        contentaddressableblob::ContentAddressableBlobStore,
        keyvalue::KeyValueStore,
        messages::{BlobReadError, GetOrCreateError, StoreError},
        storeowner::StoreOwner,
    },
};

use super::messages::{
    CreateBucket, CreateBucketError, DeleteObject, DeleteObjectError, GetObject, GetObjectError,
    GetObjectResult, HeadBucket, HeadObject, HeadObjectError, ListBuckets, ListObject,
    ListObjectError, Object, PutObject, PutObjectError, S3Error,
};

#[derive(Debug)]
pub struct S3 {
    store_owner: Addr<StoreOwner>,
    replicator: Addr<Replicator>,
    s3_kv_store: Option<Addr<KeyValueStore>>,
    s3_blob_store: Option<Addr<ContentAddressableBlobStore>>,
}

/* Kv Structure
 *
 * s3_kv_store:
 *      \0buckets\0<bucket_name>: nil
 * s3_obj_kv_store:
 *      \0objects\0<bucket_name>\0<object_name_path>: nil
 *      \0objectmeta\0<bucket_name>\0<object_name_path>\0size: size in bytes
 *      \0objectmeta\0<bucket_name>\0<object_name_path>\0last_modified: u64 timestamp
 *
 */

impl S3 {
    pub fn new(store_owner: Addr<StoreOwner>, replicator: Addr<Replicator>) -> Self {
        S3 {
            store_owner,
            replicator,
            s3_kv_store: None,
            s3_blob_store: None,
        }
    }

    fn store(&self) -> Addr<KeyValueStore> {
        self.s3_kv_store.clone().unwrap()
    }

    fn blob_store(&self) -> Addr<ContentAddressableBlobStore> {
        self.s3_blob_store.clone().unwrap()
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

    fn with_bucket_store<E, F, Fut, S>(
        &self,
        bucket: String,
        not_found_error: E,
        handler: F,
    ) -> ResponseFuture<Result<S, E>>
    where
        Fut: Future<Output = Result<S, E>>,
        F: FnOnce(Addr<KeyValueStore>, Addr<Replicator>) -> Fut + 'static,
        E: From<StoreError> + 'static,
    {
        let root_store = self.store();
        let replicator = self.replicator.clone();
        let store_owner = self.store_owner.clone();
        let bucket_path = self.bucket_path(&bucket);

        Box::pin(async move {
            if root_store
                .send(crate::stores::messages::Get { key: bucket_path })
                .await
                .unwrap()?
                .is_none()
            {
                return Err(not_found_error);
            }

            match store_owner
                .send(crate::stores::messages::GetOrCreateKeyValueStore {
                    name: format!("s3metadata\0{}", bucket).to_string(),
                })
                .await
                .unwrap()
            {
                Ok(addr) => handler(addr, replicator).await,
                Err(GetOrCreateError::StoreError(e)) => return Err(e.into()),
                Err(GetOrCreateError::StoreTypeMissmatch(store, e)) => {
                    panic!("Error getting s3 meta store {}: {}", store, e)
                }
            }
        })
    }

    async fn meta_to_obj(
        store: &Addr<KeyValueStore>,
        bucket: &String,
        obj: String,
    ) -> Result<Object, StoreError> {
        let mut meta = store
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
        let md5sum = meta.remove("md5sum").unwrap_or_default();
        let sha256sum = meta.remove("sha256sum").unwrap_or_default();
        let size = meta
            .get("size")
            .map(|e| e.parse().unwrap_or_default())
            .unwrap_or_default();
        Ok(Object {
            key: obj,
            md5sum,
            sha256sum,
            last_modified,
            size,
        })
    }
}

impl Actor for S3 {
    type Context = Context<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        let store_owner = self.store_owner.clone();
        let fut = async move {
            store_owner
                .send(crate::stores::messages::GetOrCreateKeyValueStore {
                    name: "s3metadata\0root".to_string(),
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
            .wait(ctx);

        let store_owner = self.store_owner.clone();
        let fut = async move {
            store_owner
                .send(
                    crate::stores::messages::GetOrCreateContentAddressableBlobStore {
                        name: "s3data".to_string(),
                        path: "/tmp/replistore/s3data".into(),
                    },
                )
                .await
        };
        fut.into_actor(self)
            .then(|res, act, ctx| {
                match res.unwrap() {
                    Ok(addr) => act.s3_blob_store = Some(addr),
                    Err(e) => {
                        error!("Error getting blob store {:?}", e);
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
        let object_path = self.object_path(&bucket, &prefix);
        self.with_bucket_store(
            bucket.clone(),
            ListObjectError::BucketNotFound,
            |store, _| async move {
                let mut result = Vec::new();
                for obj in store
                    .send(crate::stores::messages::List {
                        prefix: object_path,
                    })
                    .await
                    .unwrap()?
                    .into_keys()
                {
                    result.push(Self::meta_to_obj(&store, &bucket, obj).await?);
                }
                Ok(result)
            },
        )
    }
}

impl Handler<HeadObject> for S3 {
    type Result = ResponseFuture<Result<Object, HeadObjectError>>;

    fn handle(&mut self, msg: HeadObject, _ctx: &mut Self::Context) -> Self::Result {
        let HeadObject { bucket, key } = msg;
        let object_path = self.object_path(&bucket, &key);
        self.with_bucket_store(
            bucket.clone(),
            HeadObjectError::BucketNotFound,
            |store, _| async move {
                let resp = store
                    .send(crate::stores::messages::Get { key: object_path })
                    .await
                    .unwrap()?;
                if resp.is_none() {
                    return Err(HeadObjectError::ObjectNotFound);
                }
                Ok(Self::meta_to_obj(&store, &bucket, key).await?)
            },
        )
    }
}

impl Handler<PutObject> for S3 {
    type Result = ResponseFuture<Result<Object, PutObjectError>>;

    fn handle(&mut self, msg: PutObject, _ctx: &mut Self::Context) -> Self::Result {
        let PutObject { bucket, key, data } = msg;
        let blob_store = self.blob_store();
        let object_path = self.object_path(&bucket, &key);
        let last_modified_path = self.objectmeta_path(&bucket, &key, "last_modified");
        let md5sum_path = self.objectmeta_path(&bucket, &key, "md5sum");
        let sha256sum_path = self.objectmeta_path(&bucket, &key, "sha256sum");
        let size_path = self.objectmeta_path(&bucket, &key, "size");
        self.with_bucket_store(
            bucket.clone(),
            PutObjectError::BucketNotFound,
            |store, replicator| async move {
                let info = blob_store
                    .send(crate::stores::messages::PutBlob {
                        data: Box::pin(data.map_err(|e| BlobReadError { msg: e.msg })),
                    })
                    .await
                    .unwrap()?;

                let last_modified = OffsetDateTime::now_utc();
                store
                    .send(crate::stores::messages::MultiSet {
                        data: HashMap::from([
                            (object_path.clone(), String::new()),
                            (
                                last_modified_path,
                                last_modified.unix_timestamp().to_string(),
                            ),
                            (md5sum_path, info.md5sum.clone()),
                            (sha256sum_path, info.sha256sum.clone()),
                            (size_path, info.size.to_string()),
                        ]),
                    })
                    .await
                    .unwrap()?;

                replicator.do_send(ReplicateEvent {
                    bucket_event: BucketEvent {
                        bucket,
                        events: vec![Event::Put {
                            name: key.clone(),
                            meta: ObjectMeta {
                                last_modified,
                                size: info.size,
                                md5sum: info.md5sum.clone(),
                                sha256sum: info.sha256sum.clone(),
                            },
                        }],
                    },
                });

                Ok(Object {
                    key,
                    md5sum: info.md5sum,
                    sha256sum: info.sha256sum,
                    size: info.size,
                    last_modified,
                })
            },
        )
    }
}

impl Handler<GetObject> for S3 {
    type Result = ResponseFuture<Result<GetObjectResult, GetObjectError>>;

    fn handle(&mut self, msg: GetObject, _ctx: &mut Self::Context) -> Self::Result {
        let GetObject { bucket, key } = msg;
        let blob_store = self.blob_store();
        let object_path = self.object_path(&bucket, &key);
        self.with_bucket_store(
            bucket.clone(),
            GetObjectError::BucketNotFound,
            |store, _| async move {
                let resp = store
                    .send(crate::stores::messages::Get { key: object_path })
                    .await
                    .unwrap()?;
                if resp.is_none() {
                    return Err(GetObjectError::ObjectNotFound);
                }
                let meta = Self::meta_to_obj(&store, &bucket, key).await?;

                let resp = blob_store
                    .send(crate::stores::messages::GetBlob {
                        sha256sum: meta.sha256sum.clone(),
                    })
                    .await
                    .unwrap()?;

                Ok(GetObjectResult {
                    metadata: meta,
                    data: Box::pin(resp.map_err(|e| super::messages::ReadDataError { msg: e.msg })),
                })
            },
        )
    }
}

impl Handler<DeleteObject> for S3 {
    type Result = ResponseFuture<Result<(), DeleteObjectError>>;

    fn handle(&mut self, msg: DeleteObject, _ctx: &mut Self::Context) -> Self::Result {
        let DeleteObject { bucket, key } = msg;
        let blob_store = self.blob_store();
        let object_path = self.object_path(&bucket, &key);
        self.with_bucket_store(
            bucket.clone(),
            DeleteObjectError::BucketNotFound,
            |store: Addr<KeyValueStore>, replicator: Addr<Replicator>| async move {
                let resp = store
                    .send(crate::stores::messages::Get {
                        key: object_path.clone(),
                    })
                    .await
                    .unwrap()?;
                if resp.is_none() {
                    return Err(DeleteObjectError::ObjectNotFound);
                }
                let meta = Self::meta_to_obj(&store, &bucket, key.clone()).await?;

                store
                    .send(crate::stores::messages::MultiDelete {
                        data: vec![
                            object_path,
                            vec![
                                "objectmeta".to_string(),
                                bucket.clone(),
                                key.clone(),
                                String::new(),
                            ],
                        ],
                    })
                    .await
                    .unwrap()?;

                replicator.do_send(ReplicateEvent {
                    bucket_event: BucketEvent {
                        bucket,
                        events: vec![Event::Delete { name: key }],
                    },
                });

                blob_store
                    .send(crate::stores::messages::DeleteBlob {
                        sha256sum: meta.sha256sum,
                    })
                    .await
                    .unwrap()?;

                Ok(())
            },
        )
    }
}
