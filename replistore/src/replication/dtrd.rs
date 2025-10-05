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
use dtrd_client::Client;
use log::info;
use prost::Message;

use crate::replication::messages::{EventReplicationReceived, proto::BucketEvent};

use super::{Replicator, messages::ReplicateEvent};

#[derive(Debug)]
pub struct DtrdClient {
    url: String,
    endpoint: String,
    repl_target: String,
    client: Option<Client>,
    replicator: Addr<Replicator>,
}

impl DtrdClient {
    pub fn new(
        url: String,
        endpoint: String,
        repl_target: String,
        replicator: Addr<Replicator>,
    ) -> Self {
        DtrdClient {
            url,
            endpoint,
            repl_target,
            client: None,
            replicator,
        }
    }
}

impl Actor for DtrdClient {
    type Context = Context<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        let url = self.url.clone();
        let fut = async move {
            info!("Connecting to dtrd at \"{}\"", &url);
            Client::new(&url).await.unwrap()
        };

        fut.into_actor(self)
            .then(move |mut ret, act, _ctx| {
                act.client = Some(ret.clone());
                let endpoint = act.endpoint.clone();
                info!("Connected to dtrd");
                let fut = async move { ret.listen_bundles(&endpoint).await };
                fut.into_actor(act)
            })
            .then(move |ret, _act, ctx| {
                ctx.add_stream(ret.unwrap());
                info!("Reading bundles");
                fut::ready(())
            })
            .wait(ctx);
    }
}

impl Handler<ReplicateEvent> for DtrdClient {
    type Result = ();

    fn handle(&mut self, msg: ReplicateEvent, ctx: &mut Self::Context) -> Self::Result {
        let ReplicateEvent { bucket_event } = msg;
        let mut buf = BytesMut::new();
        bucket_event.encode(&mut buf).unwrap();
        let mut client = self.client.as_ref().unwrap().clone();
        let target = self.repl_target.clone();
        let fut = async move {
            client
                .submit_bundle(&target, 30, &buf, false)
                .await
                .unwrap();
        };
        fut.into_actor(self).wait(ctx);
    }
}

impl StreamHandler<Result<Vec<u8>, dtrd_client::error::Error>> for DtrdClient {
    fn handle(
        &mut self,
        item: Result<Vec<u8>, dtrd_client::error::Error>,
        ctx: &mut Self::Context,
    ) {
        let buf = Bytes::from(item.unwrap());
        let event = BucketEvent::decode(buf);
        info!("Received Event {event:?}");
        self.replicator
            .send(EventReplicationReceived {
                store_event: event.unwrap(),
            })
            .into_actor(self)
            .then(move |res, _act, _ctx| {
                info!("Event result {:?}", res.unwrap());
                fut::ready(())
            })
            .spawn(ctx);
    }
}
