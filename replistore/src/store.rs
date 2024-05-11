use std::{collections::HashMap, io::ErrorKind, path::PathBuf, pin::Pin, sync::Arc};

use bytes::{Bytes, BytesMut};
use futures_util::{AsyncWriteExt, Sink, Stream, StreamExt, TryStreamExt};
use log::info;
use md5::{Digest, Md5};
use time::OffsetDateTime;
use tokio::{fs, io::AsyncReadExt, sync::RwLock};
use tokio_util::{
    codec::{BytesCodec, FramedRead},
    compat::TokioAsyncWriteCompatExt,
};
use tracing::instrument;

#[derive(Debug)]
pub struct Store {
    base_path: PathBuf,
    buckets: RwLock<HashMap<String, Arc<Bucket>>>,
}

impl Store {
    pub fn new(path: &str) -> Self {
        let base_path = PathBuf::from(path);
        assert!(base_path.is_dir(), "basepath does not exist");
        Store {
            base_path,
            buckets: RwLock::new(HashMap::new()),
        }
    }

    #[instrument]
    pub async fn load(&self) -> Result<(), std::io::Error> {
        info!("Loading store at {}", &self.base_path.display());
        let mut buckets = self.buckets.write().await;
        let mut dirs = fs::read_dir(&self.base_path).await?;
        while let Some(entry) = dirs.next_entry().await? {
            let metadata = entry.metadata().await?;
            if metadata.is_dir() {
                let file_name = entry.file_name();
                let bucket_name = file_name.to_str().unwrap();
                let bucket = Bucket::load(&self.base_path, bucket_name).await?;
                buckets.insert(bucket_name.to_string(), Arc::new(bucket));
            }
        }
        Ok(())
    }

    #[instrument]
    pub async fn get_bucket(&self, name: &str) -> Option<Arc<Bucket>> {
        self.buckets.read().await.get(name).cloned()
    }

    #[instrument]
    pub async fn list_buckets(&self) -> Vec<String> {
        self.buckets.read().await.keys().cloned().collect()
    }
}

#[derive(Debug)]
pub struct Bucket {
    bucket_path: PathBuf,
    name: String,
    objects: RwLock<HashMap<String, Arc<Object>>>,
}

impl Bucket {
    pub async fn new(base_path: &PathBuf, name: &str) -> Self {
        let mut bucket_path = base_path.clone();
        bucket_path.push(name);
        assert!(fs::metadata(&bucket_path)
            .await
            .is_err_and(|e| e.kind() == ErrorKind::NotFound));
        fs::create_dir(&bucket_path).await.unwrap();
        Bucket {
            bucket_path,
            name: name.to_string(),
            objects: RwLock::new(HashMap::new()),
        }
    }

    #[instrument]
    pub async fn load(base_path: &PathBuf, name: &str) -> Result<Self, std::io::Error> {
        info!("Loading bucket {}", name);
        let mut bucket_path = base_path.clone();
        bucket_path.push(name);
        assert!(fs::metadata(&bucket_path).await.is_ok_and(|m| m.is_dir()));

        let mut objects = HashMap::new();
        let mut dirs = fs::read_dir(&bucket_path).await?;
        while let Some(entry) = dirs.next_entry().await? {
            let metadata = entry.metadata().await?;
            if metadata.is_file() {
                let file_name = entry.file_name();
                let object_name = file_name.to_str().unwrap();
                let bucket = Object::load(&bucket_path, object_name).await?;
                objects.insert(object_name.to_string(), Arc::new(bucket));
            }
        }

        Ok(Bucket {
            bucket_path,
            name: name.to_string(),
            objects: RwLock::new(objects),
        })
    }

    #[instrument]
    pub async fn get_object(&self, name: &str) -> Option<Arc<Object>> {
        self.objects.read().await.get(name).cloned()
    }

    #[instrument]
    pub async fn list_objects(&self) -> Vec<Arc<Object>> {
        self.objects.read().await.values().cloned().collect()
    }

    #[instrument(skip(stream))]
    pub async fn put_object(
        &self,
        name: &str,
        stream: Pin<Box<dyn Stream<Item = Result<Bytes, std::io::Error>> + Send + Sync>>,
    ) -> Result<Arc<Object>, std::io::Error> {
        let mut prototype = ObjectPrototype::new(&self.bucket_path, name).await;
        let sink = prototype.writer().await?;
        stream.forward(sink).await?;
        let object = Arc::new(prototype.finalize().await);
        let mut objects = self.objects.write().await;
        objects.insert(name.to_string(), object.clone());
        Ok(object)
    }

    #[instrument]
    pub async fn delete_object(&self, name: &str) -> Option<Result<(), std::io::Error>> {
        let mut objects = self.objects.write().await;
        match objects.remove(name) {
            Some(object) => Some(object.delete().await),
            None => None,
        }
    }
}

#[derive(Debug)]
pub struct ObjectPrototype {
    object_path: PathBuf,
    name: String,
}

impl ObjectPrototype {
    pub async fn new(base_path: &PathBuf, name: &str) -> Self {
        let mut object_path = base_path.clone();
        object_path.push(name);
        ObjectPrototype {
            object_path,
            name: name.to_string(),
        }
    }

    #[instrument]
    pub async fn writer(
        &mut self,
    ) -> Result<Pin<Box<dyn Sink<Bytes, Error = std::io::Error> + Send + Sync>>, std::io::Error>
    {
        if let Err(e) = fs::metadata(self.object_path.parent().unwrap()).await {
            if matches!(e.kind(), std::io::ErrorKind::NotFound) {
                fs::create_dir_all(self.object_path.parent().unwrap()).await?;
            }
        }
        let file = fs::File::create(&self.object_path).await?;
        let sink = AsyncWriteExt::into_sink(file.compat_write());
        Ok(Box::pin(sink))
    }

    pub async fn finalize(self) -> Object {
        Object::load_from_path(self.object_path, &self.name)
            .await
            .unwrap()
    }
}

#[derive(Debug)]
pub struct Object {
    object_path: PathBuf,
    name: String,
    size: u64,
    hashes: Hashes,
    last_modified: OffsetDateTime,
}

impl Object {
    async fn load(base_path: &PathBuf, name: &str) -> Result<Self, std::io::Error> {
        info!("Loading object {}", name);
        let mut object_path = base_path.clone();
        object_path.push(name);
        Object::load_from_path(object_path, name).await
    }

    async fn load_from_path(object_path: PathBuf, name: &str) -> Result<Self, std::io::Error> {
        let metadata = fs::metadata(&object_path).await?;
        assert!(metadata.is_file());
        Ok(Object {
            object_path: object_path.clone(),
            name: name.to_string(),
            size: metadata.len(),
            hashes: Hashes::load_from_path(&object_path).await?,
            last_modified: metadata.modified().unwrap().into(),
        })
    }

    pub fn get_name(&self) -> &str {
        &self.name
    }

    pub fn get_size(&self) -> u64 {
        self.size
    }

    pub fn get_hashes(&self) -> &Hashes {
        &self.hashes
    }

    pub fn get_last_modified(&self) -> &OffsetDateTime {
        &self.last_modified
    }

    pub async fn delete(&self) -> Result<(), std::io::Error> {
        fs::remove_file(&self.object_path).await
    }

    pub async fn read(
        &self,
    ) -> Result<
        Pin<Box<dyn Stream<Item = Result<Bytes, std::io::Error>> + Send + Sync>>,
        std::io::Error,
    > {
        let file = fs::File::open(&self.object_path).await?;
        let stream = FramedRead::new(file, BytesCodec::new()).map_ok(BytesMut::freeze);
        Ok(Box::pin(stream))
    }
}

#[derive(Debug)]
pub struct Hashes {
    md5sum: String,
    sha2_256sum: String,
}

impl Hashes {
    pub fn get_md5sum(&self) -> &str {
        &self.md5sum
    }

    pub fn get_sha2_256sum(&self) -> &str {
        &self.sha2_256sum
    }

    async fn load_from_path(path: &PathBuf) -> Result<Self, std::io::Error> {
        let mut file = fs::File::open(path).await?;
        let mut buf = vec![0; 65536];
        let mut md5_hash = Md5::new();
        let mut sha2_256_hash = sha2::Sha256::new();
        loop {
            let nread = file.read(&mut buf).await?;
            if nread == 0 {
                break;
            }
            md5_hash.update(&buf[..nread]);
            sha2_256_hash.update(&buf[..nread]);
        }
        let md5sum = hex::encode(md5_hash.finalize());
        let sha2_256sum = hex::encode(sha2_256_hash.finalize());
        Ok(Hashes {
            md5sum,
            sha2_256sum,
        })
    }
}
