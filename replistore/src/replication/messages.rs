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

use crate::stores::messages::StoreType;

#[derive(Debug)]
pub enum Event {
    Set { key: Vec<String>, value: String },
    Delete { key: Vec<String> },
    PrefixDelete { prefix: Vec<String> },
    DeleteBlob { hash: String },
}

#[derive(Debug)]
pub struct StoreEvent {
    pub store: String,
    pub store_type: StoreType,
    pub events: Vec<Event>,
}

#[derive(Message)]
#[rtype(result = "()")]
pub struct ReplicateEvent {
    pub store_event: StoreEvent,
}

#[derive(Message)]
#[rtype(result = "()")]
pub struct EventReplicationReceived {
    pub store_event: StoreEvent,
}
