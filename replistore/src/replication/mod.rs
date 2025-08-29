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

mod dtrd;
pub mod messages;

use actix::prelude::*;
use dtrd::DtrdClient;
use messages::{EventReplicationReceived, ReplicateEvent, SetEventReceiver};

use crate::{common::settings::Settings, frontend::s3::s3::ReceiveEventError};

#[derive(Debug)]
pub struct Replicator {
    client: Option<Addr<DtrdClient>>,
    receiver: Option<Recipient<EventReplicationReceived>>,
    settings: Settings,
}

impl Replicator {
    pub fn new(settings: &Settings) -> Self {
        Replicator {
            client: None,
            receiver: None,
            settings: settings.clone(),
        }
    }
}

impl Actor for Replicator {
    type Context = Context<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        let client = DtrdClient::new(
            self.settings.dtrd_url.clone(),
            self.settings.dtn_endpoint.clone(),
            self.settings.repl_target.clone(),
            ctx.address(),
        )
        .start();
        self.client = Some(client);
    }
}

impl Handler<ReplicateEvent> for Replicator {
    type Result = ();

    fn handle(&mut self, msg: ReplicateEvent, _ctx: &mut Self::Context) -> Self::Result {
        self.client.as_ref().unwrap().do_send(msg);
    }
}

impl Handler<SetEventReceiver> for Replicator {
    type Result = ();

    fn handle(&mut self, msg: SetEventReceiver, _ctx: &mut Self::Context) -> Self::Result {
        self.receiver = Some(msg.recipient);
    }
}

impl Handler<EventReplicationReceived> for Replicator {
    type Result = ResponseFuture<Result<(), ReceiveEventError>>;

    fn handle(&mut self, msg: EventReplicationReceived, _ctx: &mut Self::Context) -> Self::Result {
        let receiver = self.receiver.clone().unwrap();
        Box::pin(async move { receiver.send(msg).await.unwrap() })
    }
}
