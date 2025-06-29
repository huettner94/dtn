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
use bytes::{Bytes, BytesMut};
use futures::{SinkExt, StreamExt, TryStreamExt};
use md5::Md5;
use rocksdb::TransactionDB;
use sha2::Digest;
use std::{path::PathBuf, pin::Pin, sync::Arc};
use tokio::io::AsyncReadExt;
use tokio_util::{
    codec::{BytesCodec, FramedRead},
    compat::TokioAsyncWriteCompatExt,
};

use crate::replication::{
    messages::{Event, ReplicateEvent, StoreEvent},
    Replicator,
};

use super::messages::{
    BlobInfo, BlobReadError, DeleteBlob, DeleteBlobError, GetBlob, GetBlobError, PutBlob,
    PutBlobError, StoreType,
};

pub struct ContentAddressableBlobStore {
    name: String,
    base_path: PathBuf,
    db: Arc<TransactionDB>,
    replicator: Addr<Replicator>,
}

impl std::fmt::Debug for ContentAddressableBlobStore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ContentAddressableBlobStore")
            .field("name", &self.name)
            .finish()
    }
}

impl ContentAddressableBlobStore {
    pub fn new(
        name: String,
        base_path: PathBuf,
        db: Arc<TransactionDB>,
        replicator: Addr<Replicator>,
    ) -> Self {
        ContentAddressableBlobStore {
            name,
            base_path,
            db,
            replicator,
        }
    }

    fn get_path(&self, keys: Vec<String>) -> String {
        format!("\0store\0{}\0{}", self.name, keys.join("\0"))
    }

    fn get_disk_base_path(&self) -> PathBuf {
        self.base_path.join("data")
    }

    fn get_disk_path(&self, sha256sum: &str) -> PathBuf {
        self.get_disk_base_path().join(sha256sum)
    }

    fn get_disk_tmp_path(&self) -> PathBuf {
        let uuid = uuid::Uuid::new_v4().to_string();
        self.get_disk_base_path().join("tmp").join(uuid)
    }

    async fn hash_file(path: &PathBuf) -> Result<(String, String), std::io::Error> {
        let mut file = tokio::fs::File::open(path).await?;
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
        Ok((md5sum, sha2_256sum))
    }

    fn send_event(&self, event: Event) {
        self.replicator.do_send(ReplicateEvent {
            store_event: StoreEvent {
                store: self.name.clone(),
                store_type: StoreType::ContentAddressableBlob,
                events: vec![event],
            },
        });
    }
}

impl Actor for ContentAddressableBlobStore {
    type Context = Context<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        let fullpath = self.base_path.join("data").join("tmp");
        let fut = async move { tokio::fs::create_dir_all(&fullpath).await.unwrap() };

        fut.into_actor(self).wait(ctx)
    }
}

impl Handler<PutBlob> for ContentAddressableBlobStore {
    type Result = ResponseActFuture<Self, Result<BlobInfo, PutBlobError>>;

    fn handle(&mut self, msg: PutBlob, _ctx: &mut Context<Self>) -> Self::Result {
        let PutBlob { data } = msg;
        let basedir = self.get_disk_base_path();
        let tmpfile = self.get_disk_tmp_path();

        Box::pin(
            async move {
                let file = Box::pin(
                    futures_util::AsyncWriteExt::into_sink(
                        tokio::fs::File::create(&tmpfile).await?.compat_write(),
                    )
                    .sink_map_err(|e| e.into()),
                );

                data.map_err(|e| Into::<PutBlobError>::into(e))
                    .forward(file)
                    .await?;

                let (md5sum, sha256sum) = Self::hash_file(&tmpfile).await?;

                let target_name = basedir.join(&sha256sum);
                tokio::fs::rename(&tmpfile, &target_name).await?;

                let size = tokio::fs::metadata(&target_name).await?.len();

                Ok(BlobInfo {
                    md5sum,
                    sha256sum,
                    size,
                })
            }
            .into_actor(self) // converts future to ActorFuture
            .map(|res, _act, _ctx| res),
        )
    }
}

impl Handler<GetBlob> for ContentAddressableBlobStore {
    type Result = ResponseFuture<
        Result<
            Pin<Box<dyn Stream<Item = Result<Bytes, BlobReadError>> + Send + Sync>>,
            GetBlobError,
        >,
    >;

    fn handle(&mut self, msg: GetBlob, _ctx: &mut Self::Context) -> Self::Result {
        let GetBlob { sha256sum } = msg;
        let filepath = self.get_disk_path(&sha256sum);

        Box::pin(async move {
            let metadata = tokio::fs::metadata(&filepath).await?;
            if !metadata.is_file() {
                return Err(GetBlobError::BlobDoesNotExist);
            }

            let file = tokio::fs::File::open(&filepath).await?;
            let stream = FramedRead::new(file, BytesCodec::new())
                .map_ok(BytesMut::freeze)
                .map_err(|e| BlobReadError { msg: e.to_string() });

            // need a explicit type here, otherwise daemons will arise
            let out: Result<
                Pin<Box<dyn Stream<Item = Result<Bytes, BlobReadError>> + Send + Sync>>,
                GetBlobError,
            > = Ok(Box::pin(stream));
            out
        })
    }
}

impl Handler<DeleteBlob> for ContentAddressableBlobStore {
    type Result = ResponseFuture<Result<(), DeleteBlobError>>;

    fn handle(&mut self, msg: DeleteBlob, _ctx: &mut Self::Context) -> Self::Result {
        let DeleteBlob { sha256sum } = msg;
        let filepath = self.get_disk_path(&sha256sum);

        self.send_event(Event::DeleteBlob { hash: sha256sum });

        Box::pin(async move {
            let metadata = tokio::fs::metadata(&filepath).await?;
            if !metadata.is_file() {
                return Err(DeleteBlobError::BlobDoesNotExist);
            }

            tokio::fs::remove_file(&filepath).await?;
            Ok(())
        })
    }
}
