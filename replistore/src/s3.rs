use std::time;

use async_trait::async_trait;
use futures_util::TryStreamExt;
use log::debug;
use s3s::{
    dto::{
        Bucket, DeleteObjectInput, DeleteObjectOutput, GetBucketLocationInput,
        GetBucketLocationOutput, GetObjectInput, GetObjectOutput, HeadBucketInput,
        HeadBucketOutput, HeadObjectInput, HeadObjectOutput, ListBucketsInput, ListBucketsOutput,
        ListObjectsInput, ListObjectsOutput, ListObjectsV2Input, ListObjectsV2Output, Object,
        Owner, PutObjectInput, PutObjectOutput,
    },
    s3_error, S3Request, S3Response, S3Result, S3,
};
use tracing::instrument;

use crate::store::Store;

#[derive(Debug)]
pub struct FileStore {
    store: Store,
}

impl FileStore {
    pub fn new(store: Store) -> Self {
        FileStore { store }
    }
}

#[async_trait]
impl S3 for FileStore {
    #[instrument]
    async fn get_bucket_location(
        &self,
        _req: S3Request<GetBucketLocationInput>,
    ) -> S3Result<S3Response<GetBucketLocationOutput>> {
        debug!("get_bucket_location");
        Ok(S3Response::new(GetBucketLocationOutput {
            location_constraint: None,
        }))
    }

    #[instrument]
    async fn list_buckets(
        &self,
        _req: S3Request<ListBucketsInput>,
    ) -> S3Result<S3Response<ListBucketsOutput>> {
        debug!("list_buckets");
        let buckets: Vec<Bucket> = self
            .store
            .list_buckets()
            .await
            .iter()
            .map(|name| Bucket {
                creation_date: Some(time::SystemTime::now().into()),
                name: Some(name.to_string()),
            })
            .collect();
        Ok(S3Response::new(ListBucketsOutput {
            buckets: Some(buckets),
            owner: Some(Owner {
                display_name: Some("ich teste mal".to_string()),
                id: Some("test".to_string()),
            }),
        }))
    }

    #[instrument]
    async fn head_bucket(
        &self,
        _req: S3Request<HeadBucketInput>,
    ) -> S3Result<S3Response<HeadBucketOutput>> {
        debug!("head_bucket {}", _req.input.bucket);
        match self.store.get_bucket(&_req.input.bucket).await {
            Some(_) => Ok(S3Response::new(HeadBucketOutput {})),
            None => Err(s3_error!(NoSuchBucket)),
        }
    }

    #[instrument]
    async fn list_objects(
        &self,
        _req: S3Request<ListObjectsInput>,
    ) -> S3Result<S3Response<ListObjectsOutput>> {
        debug!("list_objects {}", _req.input.bucket);
        match self.store.get_bucket(&_req.input.bucket).await {
            Some(bucket) => {
                let objects: Vec<Object> = bucket
                    .list_objects()
                    .await
                    .iter()
                    .filter(|e| match &_req.input.prefix {
                        Some(prefix) => e.get_name().starts_with(prefix),
                        None => true,
                    })
                    .map(|object| Object {
                        key: Some(object.get_name().to_string()),
                        last_modified: Some((*object.get_last_modified()).into()),
                        size: object.get_size() as i64,
                        ..Default::default()
                    })
                    .collect();
                Ok(S3Response::new(ListObjectsOutput {
                    contents: Some(objects),
                    max_keys: i32::MAX,
                    name: Some(_req.input.bucket),
                    ..Default::default()
                }))
            }
            None => Err(s3_error!(NoSuchBucket)),
        }
    }

    #[instrument]
    async fn list_objects_v2(
        &self,
        _req: S3Request<ListObjectsV2Input>,
    ) -> S3Result<S3Response<ListObjectsV2Output>> {
        debug!("list_objects_v2 {}", _req.input.bucket);
        match self.store.get_bucket(&_req.input.bucket).await {
            Some(bucket) => {
                let objects: Vec<Object> = bucket
                    .list_objects()
                    .await
                    .iter()
                    .filter(|e| match &_req.input.prefix {
                        Some(prefix) => e.get_name().starts_with(prefix),
                        None => true,
                    })
                    .map(|object| Object {
                        key: Some(object.get_name().to_string()),
                        last_modified: Some((*object.get_last_modified()).into()),
                        size: object.get_size() as i64,
                        ..Default::default()
                    })
                    .collect();
                Ok(S3Response::new(ListObjectsV2Output {
                    contents: Some(objects),
                    max_keys: i32::MAX,
                    name: Some(_req.input.bucket),
                    ..Default::default()
                }))
            }
            None => Err(s3_error!(NoSuchBucket)),
        }
    }

    #[instrument]
    async fn get_object(
        &self,
        _req: S3Request<GetObjectInput>,
    ) -> S3Result<S3Response<GetObjectOutput>> {
        debug!("get_object {} {}", _req.input.bucket, _req.input.key);
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
        debug!("head_object {} {}", _req.input.bucket, _req.input.key);
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
        debug!("delete_object {} {}", _req.input.bucket, _req.input.key);
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
    }

    #[instrument]
    async fn put_object(
        &self,
        _req: S3Request<PutObjectInput>,
    ) -> S3Result<S3Response<PutObjectOutput>> {
        debug!("put_object {} {}", _req.input.bucket, _req.input.key);
        match self.store.get_bucket(&_req.input.bucket).await {
            Some(bucket) => {
                let body = _req.input.body.unwrap();
                let body_stream =
                    Box::pin(body.map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e)));
                let object = bucket
                    .put_object(&_req.input.key, body_stream)
                    .await
                    .unwrap();
                Ok(S3Response::new(PutObjectOutput {
                    e_tag: Some(object.get_hashes().get_md5sum().to_string()),
                    checksum_sha256: Some(object.get_hashes().get_sha2_256sum().to_string()),
                    ..Default::default()
                }))
            }
            None => Err(s3_error!(NoSuchBucket)),
        }
    }
}
