// Copyright (C) 2025 Felix Huettner
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
use messages::ReplicateEvent;

#[derive(Debug)]
pub struct Replicator {
    client: Addr<DtrdClient>,
}

impl Replicator {
    pub fn new() -> Self {
        Replicator {
            client: DtrdClient::new("http://localhost:50051".to_string()).start(),
        }
    }
}

impl Actor for Replicator {
    type Context = Context<Self>;
}

impl Handler<ReplicateEvent> for Replicator {
    type Result = ();

    fn handle(&mut self, msg: ReplicateEvent, ctx: &mut Self::Context) -> Self::Result {
        self.client.do_send(msg);
    }
}
