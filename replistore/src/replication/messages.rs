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

use crate::{
    frontend::s3::s3_backend::ReceiveEventError, replication::messages::proto::BucketEvent,
};
use actix::prelude::*;

#[allow(clippy::all, clippy::pedantic, clippy::restriction, clippy::nursery)]
pub mod proto {
    include!(concat!(env!("OUT_DIR"), "/replication.messages.rs"));
}

#[derive(Message)]
#[rtype(result = "()")]
pub struct ReplicateEvent {
    pub bucket_event: BucketEvent,
}

#[derive(Message)]
#[rtype(result = "()")]
pub struct SetEventReceiver {
    pub recipient: Recipient<EventReplicationReceived>,
}

#[derive(Message)]
#[rtype(result = "Result<(), ReceiveEventError>")]
pub struct EventReplicationReceived {
    pub store_event: BucketEvent,
}
