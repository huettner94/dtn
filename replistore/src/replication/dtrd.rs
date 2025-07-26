use actix::prelude::*;
use bytes::{Bytes, BytesMut};
use dtrd_client::Client;
use log::{error, info};
use prost::Message;

use super::messages::{BucketEvent, ReplicateEvent};

#[derive(Debug)]
pub struct DtrdClient {
    url: String,
    client: Option<Client>,
}

impl DtrdClient {
    pub fn new(url: String) -> Self {
        DtrdClient { url, client: None }
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
                info!("Connected to dtrd");
                let fut = async move { ret.listen_bundles("dtn://defaultnodeid/myendpoint").await };
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
        let fut = async move {
            client
                .submit_bundle("dtn://defaultnodeid/myendpoint", 30, &buf)
                .await
                .unwrap()
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
        println!("{:?}", event.unwrap());
    }
}
