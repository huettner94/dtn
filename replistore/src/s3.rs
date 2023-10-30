use std::time;

use async_trait::async_trait;
use futures_util::TryStreamExt;
use s3s::{
    dto::{
        Bucket, DeleteObjectInput, DeleteObjectOutput, GetObjectInput, GetObjectOutput,
        HeadObjectInput, HeadObjectOutput, ListBucketsInput, ListBucketsOutput, ListObjectsInput,
        ListObjectsOutput, Object, Owner, PutObjectInput, PutObjectOutput,
    },
    s3_error, S3Request, S3Response, S3Result, S3,
};

use crate::store::Store;

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
    async fn list_buckets(
        &self,
        _req: S3Request<ListBucketsInput>,
    ) -> S3Result<S3Response<ListBucketsOutput>> {
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

    async fn list_objects(
        &self,
        _req: S3Request<ListObjectsInput>,
    ) -> S3Result<S3Response<ListObjectsOutput>> {
        match self.store.get_bucket(&_req.input.bucket).await {
            Some(bucket) => {
                let objects: Vec<Object> = bucket
                    .list_objects()
                    .await
                    .iter()
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
                        ..Default::default()
                    }))
                }
                None => Err(s3_error!(NoSuchKey)),
            },
            None => Err(s3_error!(NoSuchBucket)),
        }
    }

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
                    ..Default::default()
                })),
                None => Err(s3_error!(NoSuchKey)),
            },
            None => Err(s3_error!(NoSuchBucket)),
        }
    }

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
    }

    async fn put_object(
        &self,
        _req: S3Request<PutObjectInput>,
    ) -> S3Result<S3Response<PutObjectOutput>> {
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
                    ..Default::default()
                }))
            }
            None => Err(s3_error!(NoSuchBucket)),
        }
    }
}
