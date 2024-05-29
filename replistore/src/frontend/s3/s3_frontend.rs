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
use futures_util::future::FutureExt;

use ::time::OffsetDateTime;
use hyper::Server;
use log::info;
use s3s::{
    auth::SimpleAuth,
    dto::{CreateBucketInput, CreateBucketOutput},
    service::S3ServiceBuilder,
};
use tokio::sync::{broadcast, mpsc};

use std::time;

use async_trait::async_trait;
use s3s::{
    dto::{
        Bucket, DeleteObjectInput, DeleteObjectOutput, GetBucketLocationInput,
        GetBucketLocationOutput, GetObjectInput, GetObjectOutput, HeadBucketInput,
        HeadBucketOutput, HeadObjectInput, HeadObjectOutput, ListBucketsInput, ListBucketsOutput,
        ListObjectsInput, ListObjectsOutput, ListObjectsV2Input, ListObjectsV2Output, Object,
        Owner, PutObjectInput, PutObjectOutput,
    },
    s3_error, S3Request, S3Response, S3Result,
};
use tracing::instrument;

use super::messages::CreateBucketError;

impl From<super::messages::S3Error> for s3s::S3Error {
    fn from(value: super::messages::S3Error) -> Self {
        s3s::S3Error::with_message(s3s::S3ErrorCode::InternalError, format!("{:?}", value))
    }
}

impl From<super::messages::CreateBucketError> for s3s::S3Error {
    fn from(value: super::messages::CreateBucketError) -> Self {
        match value {
            super::messages::CreateBucketError::S3Error(e) => e.into(),
            super::messages::CreateBucketError::BucketAlreadyExists => {
                s3_error!(BucketAlreadyExists)
            }
        }
    }
}

impl From<super::messages::ListObjectError> for s3s::S3Error {
    fn from(value: super::messages::ListObjectError) -> Self {
        match value {
            super::messages::ListObjectError::S3Error(e) => e.into(),
            super::messages::ListObjectError::BucketNotFound => s3_error!(NoSuchBucket),
        }
    }
}

impl From<super::messages::PutObjectError> for s3s::S3Error {
    fn from(value: super::messages::PutObjectError) -> Self {
        match value {
            super::messages::PutObjectError::S3Error(e) => e.into(),
            super::messages::PutObjectError::BucketNotFound => s3_error!(NoSuchBucket),
        }
    }
}

#[async_trait]
trait AddrExt<A> {
    async fn send_s3<M>(&self, msg: M) -> Result<M::Result, s3s::S3Error>
    where
        M: Message + Send + 'static,
        M::Result: Send,
        A: Handler<M>,
        A::Context: actix::dev::ToEnvelope<A, M>;
}

#[async_trait]
impl<A: actix::Actor> AddrExt<A> for Addr<A> {
    async fn send_s3<M>(&self, msg: M) -> Result<M::Result, s3s::S3Error>
    where
        M: Message + Send + 'static,
        M::Result: Send,
        A: Handler<M>,
        A::Context: actix::dev::ToEnvelope<A, M>,
    {
        self.send(msg)
            .await
            .map_err(|_| s3s::S3Error::with_message(s3s::S3ErrorCode::InternalError, "actix error"))
    }
}

#[derive(Debug)]
pub struct S3Frontend {
    s3: Addr<super::s3::S3>,
}

impl S3Frontend {
    pub async fn new(s3: Addr<super::s3::S3>) -> Self {
        S3Frontend { s3 }
    }

    pub async fn run(
        self,
        mut shutdown: broadcast::Receiver<()>,
        _shutdown_complete_sender: mpsc::Sender<()>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        // Setup S3Frontend service
        let service = {
            let mut b = S3ServiceBuilder::new(self);

            // Enable authentication
            b.set_auth(SimpleAuth::from_single("cake", "ilike"));

            b.build()
        };

        // Run server
        let addr = "0.0.0.0:8080".parse().unwrap();
        info!("Server listening on {}", addr);
        let server = Server::try_bind(&addr)
            .unwrap()
            .serve(service.into_shared().into_make_service());

        info!("server is running at http://{addr}");
        server
            .with_graceful_shutdown(shutdown.recv().map(|_| ()))
            .await?;

        info!("Server has shutdown. See you");
        // _shutdown_complete_sender is explicitly dropped here
        Ok(())
    }
}

#[async_trait]
impl s3s::S3 for S3Frontend {
    #[instrument]
    async fn get_bucket_location(
        &self,
        _req: S3Request<GetBucketLocationInput>,
    ) -> S3Result<S3Response<GetBucketLocationOutput>> {
        Ok(S3Response::new(GetBucketLocationOutput {
            location_constraint: None,
        }))
    }

    #[instrument]
    async fn list_buckets(
        &self,
        _req: S3Request<ListBucketsInput>,
    ) -> S3Result<S3Response<ListBucketsOutput>> {
        let buckets = self.s3.send_s3(super::messages::ListBuckets {}).await??;

        Ok(S3Response::new(ListBucketsOutput {
            buckets: Some(
                buckets
                    .into_iter()
                    .map(|name| Bucket {
                        creation_date: Some(time::SystemTime::now().into()),
                        name: Some(name.to_string()),
                    })
                    .collect(),
            ),
            owner: Some(Owner {
                display_name: Some("ich teste mal".to_string()),
                id: Some("test".to_string()),
            }),
        }))
    }

    #[instrument]
    async fn create_bucket(
        &self,
        req: S3Request<CreateBucketInput>,
    ) -> S3Result<S3Response<CreateBucketOutput>> {
        let bucket = self
            .s3
            .send_s3(super::messages::CreateBucket {
                name: req.input.bucket,
            })
            .await??;
        Ok(S3Response::new(CreateBucketOutput {
            location: Some(format!("/{}", bucket)),
        }))
    }

    #[instrument]
    async fn head_bucket(
        &self,
        req: S3Request<HeadBucketInput>,
    ) -> S3Result<S3Response<HeadBucketOutput>> {
        match self
            .s3
            .send_s3(super::messages::HeadBucket {
                name: req.input.bucket,
            })
            .await??
        {
            Some(_) => Ok(S3Response::new(HeadBucketOutput {})),
            None => Err(s3_error!(NoSuchBucket)),
        }
    }

    #[instrument]
    async fn list_objects(
        &self,
        req: S3Request<ListObjectsInput>,
    ) -> S3Result<S3Response<ListObjectsOutput>> {
        let objects = self
            .s3
            .send_s3(super::messages::ListObject {
                bucket: req.input.bucket.clone(),
                prefix: req.input.prefix.unwrap_or_default(),
            })
            .await??;

        Ok(S3Response::new(ListObjectsOutput {
            contents: Some(
                objects
                    .into_iter()
                    .map(|obj| Object {
                        key: Some(obj.key),
                        last_modified: Some(OffsetDateTime::now_utc().into()),
                        size: 0,
                        ..Default::default()
                    })
                    .collect(),
            ),
            max_keys: i32::MAX,
            name: Some(req.input.bucket),
            ..Default::default()
        }))
    }

    #[instrument]
    async fn list_objects_v2(
        &self,
        req: S3Request<ListObjectsV2Input>,
    ) -> S3Result<S3Response<ListObjectsV2Output>> {
        let objects = self
            .s3
            .send_s3(super::messages::ListObject {
                bucket: req.input.bucket.clone(),
                prefix: req.input.prefix.unwrap_or_default(),
            })
            .await??;

        Ok(S3Response::new(ListObjectsV2Output {
            contents: Some(
                objects
                    .into_iter()
                    .map(|obj| Object {
                        key: Some(obj.key),
                        last_modified: Some(OffsetDateTime::now_utc().into()),
                        size: 0,
                        ..Default::default()
                    })
                    .collect(),
            ),
            max_keys: i32::MAX,
            name: Some(req.input.bucket),
            ..Default::default()
        }))
    }

    /*#[instrument]
    async fn get_object(
        &self,
        _req: S3Request<GetObjectInput>,
    ) -> S3Result<S3Response<GetObjectOutput>> {
        match self.store.get_bucket(&_req.input.bucket).await {
            Some(bucket) => match bucket.get_object(&_req.input.key).await {
                Some(object) => {
                    let stream = object.read().await.unwrap();
                    Ok(S3Response::new(GetObjectOutput {
                        body: Some(s3s::dto::StreamingBlob::wrap(stream)),
                        last_modified: Some((*object.get_last_modified()).into()),
                        content_length: object.get_size() as i64,
                        e_tag: Some(object.get_hashes().get_md5sum().to_string()),
                        checksum_sha256: Some(object.get_hashes().get_sha2_256sum().to_string()),
                        ..Default::default()
                    }))
                }
                None => Err(s3_error!(NoSuchKey)),
            },
            None => Err(s3_error!(NoSuchBucket)),
        }
    }

    #[instrument]
    async fn head_object(
        &self,
        _req: S3Request<HeadObjectInput>,
    ) -> S3Result<S3Response<HeadObjectOutput>> {
        match self.store.get_bucket(&_req.input.bucket).await {
            Some(bucket) => match bucket.get_object(&_req.input.key).await {
                Some(object) => Ok(S3Response::new(HeadObjectOutput {
                    last_modified: Some((*object.get_last_modified()).into()),
                    content_length: object.get_size() as i64,
                    e_tag: Some(object.get_hashes().get_md5sum().to_string()),
                    checksum_sha256: Some(object.get_hashes().get_sha2_256sum().to_string()),
                    ..Default::default()
                })),
                None => Err(s3_error!(NoSuchKey)),
            },
            None => Err(s3_error!(NoSuchBucket)),
        }
    }

    #[instrument]
    async fn delete_object(
        &self,
        _req: S3Request<DeleteObjectInput>,
    ) -> S3Result<S3Response<DeleteObjectOutput>> {
        match self.store.get_bucket(&_req.input.bucket).await {
            Some(bucket) => match bucket.delete_object(&_req.input.key).await {
                Some(result) => {
                    result.unwrap();
                    Ok(S3Response::new(DeleteObjectOutput {
                        ..Default::default()
                    }))
                }
                None => Err(s3_error!(NoSuchKey)),
            },
            None => Err(s3_error!(NoSuchBucket)),
        }
    }*/

    #[instrument]
    async fn put_object(
        &self,
        req: S3Request<PutObjectInput>,
    ) -> S3Result<S3Response<PutObjectOutput>> {
        let object = self
            .s3
            .send_s3(super::messages::PutObject {
                bucket: req.input.bucket,
                key: req.input.key,
            })
            .await??;

        Ok(S3Response::new(PutObjectOutput {
            e_tag: Some(object.md5sum),
            checksum_sha256: Some(object.sha256sum),
            ..Default::default()
        }))
    }
}
