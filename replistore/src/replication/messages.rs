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

use actix::prelude::*;
use time::OffsetDateTime;

#[derive(Debug)]
pub struct ObjectMeta {
    pub last_modified: OffsetDateTime,
    pub size: u64,
    pub md5sum: String,
    pub sha256sum: String,
}

#[derive(Debug)]
pub enum Event {
    Put { name: String, meta: ObjectMeta },
    Delete { name: String },
}

#[derive(Debug)]
pub struct BucketEvent {
    pub bucket: String,
    pub events: Vec<Event>,
}

#[derive(Message)]
#[rtype(result = "()")]
pub struct ReplicateEvent {
    pub bucket_event: BucketEvent,
}

#[derive(Message)]
#[rtype(result = "()")]
pub struct EventReplicationReceived {
    pub store_event: BucketEvent,
}
