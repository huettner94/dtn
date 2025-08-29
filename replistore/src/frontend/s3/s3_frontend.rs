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
use futures::TryStreamExt;

use log::info;
use s3s::{
    auth::SimpleAuth,
    dto::{CreateBucketInput, CreateBucketOutput},
    service::S3ServiceBuilder,
};
use tokio::{
    net::TcpListener,
    sync::{broadcast, mpsc},
};

use hyper_util::rt::{TokioExecutor, TokioIo};
use hyper_util::server::conn::auto::Builder as ConnBuilder;

use std::time;

use async_trait::async_trait;
use s3s::{
    S3Request, S3Response, S3Result,
    dto::{
        Bucket, DeleteObjectInput, DeleteObjectOutput, GetBucketLocationInput,
        GetBucketLocationOutput, GetObjectInput, GetObjectOutput, HeadBucketInput,
        HeadBucketOutput, HeadObjectInput, HeadObjectOutput, ListBucketsInput, ListBucketsOutput,
        ListObjectsInput, ListObjectsOutput, ListObjectsV2Input, ListObjectsV2Output, Object,
        Owner, PutObjectInput, PutObjectOutput,
    },
    s3_error,
};
use tracing::instrument;

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
            super::messages::PutObjectError::ReadDataError(e) => {
                s3s::S3Error::with_message(s3s::S3ErrorCode::InternalError, e.msg)
            }
        }
    }
}

impl From<super::messages::HeadObjectError> for s3s::S3Error {
    fn from(value: super::messages::HeadObjectError) -> Self {
        match value {
            super::messages::HeadObjectError::S3Error(e) => e.into(),
            super::messages::HeadObjectError::BucketNotFound => s3_error!(NoSuchBucket),
            super::messages::HeadObjectError::ObjectNotFound => s3_error!(NoSuchKey),
        }
    }
}

impl From<super::messages::GetObjectError> for s3s::S3Error {
    fn from(value: super::messages::GetObjectError) -> Self {
        match value {
            super::messages::GetObjectError::S3Error(e) => e.into(),
            super::messages::GetObjectError::BucketNotFound => s3_error!(NoSuchBucket),
            super::messages::GetObjectError::ObjectNotFound => s3_error!(NoSuchKey),
            super::messages::GetObjectError::ReadDataError(e) => {
                s3s::S3Error::with_message(s3s::S3ErrorCode::InternalError, e.msg)
            }
        }
    }
}

impl From<super::messages::DeleteObjectError> for s3s::S3Error {
    fn from(value: super::messages::DeleteObjectError) -> Self {
        match value {
            super::messages::DeleteObjectError::S3Error(e) => e.into(),
            super::messages::DeleteObjectError::BucketNotFound => s3_error!(NoSuchBucket),
            super::messages::DeleteObjectError::ObjectNotFound => s3_error!(NoSuchKey),
            super::messages::DeleteObjectError::ReadDataError(e) => {
                s3s::S3Error::with_message(s3s::S3ErrorCode::InternalError, e.msg)
            }
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
    s3_port: u16,
}

impl S3Frontend {
    pub async fn new(s3: Addr<super::s3::S3>, s3_port: u16) -> Self {
        S3Frontend { s3, s3_port }
    }

    pub async fn run(
        self,
        mut shutdown: broadcast::Receiver<()>,
        _shutdown_complete_sender: mpsc::Sender<()>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let s3_port = self.s3_port;

        // Setup S3Frontend service
        let service = {
            let mut b = S3ServiceBuilder::new(self);

            // Enable authentication
            b.set_auth(SimpleAuth::from_single("cake", "ilike"));

            b.build()
        };

        let hyper_service = service.into_shared();

        // Run server
        let listener = TcpListener::bind(("0.0.0.0", s3_port)).await?;
        let local_addr = listener.local_addr()?;
        info!("Server listening on {}", local_addr);

        let http_server = ConnBuilder::new(TokioExecutor::new());
        let graceful = hyper_util::server::graceful::GracefulShutdown::new();

        loop {
            let (socket, _) = tokio::select! {
                res =  listener.accept() => {
                    match res {
                        Ok(conn) => conn,
                        Err(err) => {
                            tracing::error!("error accepting connection: {err}");
                            continue;
                        }
                    }
                }
                _ = shutdown.recv() => {
                    info!("Shutting down s3 frontend");
                    break;
                }
            };

            let conn = http_server.serve_connection(TokioIo::new(socket), hyper_service.clone());
            let conn = graceful.watch(conn.into_owned());
            tokio::spawn(async move {
                let _ = conn.await;
            });
        }

        tokio::select! {
            () = graceful.shutdown() => {
                 info!("Gracefully shutdown!");
            },
            () = tokio::time::sleep(std::time::Duration::from_secs(10)) => {
                 info!("Waited 10 seconds for graceful shutdown, aborting...");
            }
        }
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
            continuation_token: None,
            buckets: Some(
                buckets
                    .into_iter()
                    .map(|name| Bucket {
                        bucket_region: None,
                        creation_date: Some(time::SystemTime::now().into()),
                        name: Some(name.to_string()),
                    })
                    .collect(),
            ),
            owner: Some(Owner {
                display_name: Some("ich teste mal".to_string()),
                id: Some("test".to_string()),
            }),
            prefix: None,
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
            Some(_) => Ok(S3Response::new(HeadBucketOutput {
                ..Default::default()
            })),
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
                        last_modified: Some(obj.last_modified.into()),
                        size: Some(obj.size as i64),
                        ..Default::default()
                    })
                    .collect(),
            ),
            max_keys: None,
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
                        last_modified: Some(obj.last_modified.into()),
                        size: Some(obj.size as i64),
                        ..Default::default()
                    })
                    .collect(),
            ),
            max_keys: None,
            name: Some(req.input.bucket),
            ..Default::default()
        }))
    }

    #[instrument]
    async fn head_object(
        &self,
        req: S3Request<HeadObjectInput>,
    ) -> S3Result<S3Response<HeadObjectOutput>> {
        let obj = self
            .s3
            .send_s3(super::messages::HeadObject {
                bucket: req.input.bucket.clone(),
                key: req.input.key,
            })
            .await??;

        Ok(S3Response::new(HeadObjectOutput {
            last_modified: Some(obj.last_modified.into()),
            content_length: Some(obj.size as i64),
            e_tag: Some(obj.md5sum),
            checksum_sha256: Some(obj.sha256sum),
            ..Default::default()
        }))
    }

    #[instrument]
    async fn get_object(
        &self,
        req: S3Request<GetObjectInput>,
    ) -> S3Result<S3Response<GetObjectOutput>> {
        let result = self
            .s3
            .send_s3(super::messages::GetObject {
                bucket: req.input.bucket.clone(),
                key: req.input.key,
            })
            .await??;
        let obj = result.metadata;

        Ok(S3Response::new(GetObjectOutput {
            body: Some(s3s::dto::StreamingBlob::wrap(Box::pin(
                result.data.map_err(|e| std::io::Error::other(e.msg)),
            ))),
            last_modified: Some(obj.last_modified.into()),
            content_length: Some(obj.size as i64),
            e_tag: Some(obj.md5sum),
            checksum_sha256: Some(obj.sha256sum),
            ..Default::default()
        }))
    }

    #[instrument]
    async fn delete_object(
        &self,
        req: S3Request<DeleteObjectInput>,
    ) -> S3Result<S3Response<DeleteObjectOutput>> {
        self.s3
            .send_s3(super::messages::DeleteObject {
                bucket: req.input.bucket.clone(),
                key: req.input.key,
            })
            .await??;

        Ok(S3Response::new(DeleteObjectOutput {
            ..Default::default()
        }))
    }

    #[instrument]
    async fn put_object(
        &self,
        req: S3Request<PutObjectInput>,
    ) -> S3Result<S3Response<PutObjectOutput>> {
        if req.input.body.is_none() {
            return Err(s3_error!(InvalidRequest));
        }
        let object = self
            .s3
            .send_s3(super::messages::PutObject {
                bucket: req.input.bucket,
                key: req.input.key,
                data: Box::pin(
                    req.input
                        .body
                        .unwrap()
                        .map_err(|e| super::messages::ReadDataError { msg: e.to_string() }),
                ),
            })
            .await??;

        Ok(S3Response::new(PutObjectOutput {
            e_tag: Some(object.md5sum),
            checksum_sha256: Some(object.sha256sum),
            ..Default::default()
        }))
    }
}
