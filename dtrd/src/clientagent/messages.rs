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
    bundlestorageagent::StoredBundle, nodeagent::messages::Node,
    routingagent::messages::RouteStatus,
};
use actix::prelude::*;
use bp7::endpoint::Endpoint;
use tokio::sync::mpsc;
use url::Url;

#[derive(Message, Debug)]
#[rtype(result = "()")]
pub struct ClientDeliverBundle {
    pub bundle: StoredBundle,
    pub responder: Recipient<EventBundleDelivered>,
}

#[derive(Message)]
#[rtype(result = "()")]
pub struct EventBundleDelivered {
    pub endpoint: Endpoint,
    pub bundle: StoredBundle,
}

#[derive(Message)]
#[rtype(result = "()")]
pub struct EventBundleDeliveryFailed {
    pub endpoint: Endpoint,
    pub bundle: StoredBundle,
}

#[derive(Message)]
#[rtype(result = "()")]
pub struct EventClientConnected {
    pub destination: Endpoint,
    pub sender: Recipient<ClientDeliverBundle>,
}

#[derive(Message)]
#[rtype(result = "()")]
pub struct EventClientDisconnected {
    pub destination: Endpoint,
}

#[derive(Message)]
#[rtype(result = "Result<(), String>")]
pub struct ClientListenConnect {
    pub destination: Endpoint,
    pub sender: mpsc::Sender<ClientDeliverBundle>,
}

#[derive(Message)]
#[rtype(result = "()")]
pub struct ClientListenDisconnect {
    pub destination: Endpoint,
}

#[derive(Message)]
#[rtype(result = "Result<(), ()>")]
pub struct ClientSendBundle {
    pub destination: Endpoint,
    pub payload: Vec<u8>,
    pub lifetime: u64,
}
#[derive(Message)]
#[rtype(result = "Vec<Node>")]
pub struct ClientListNodes {}

#[derive(Message)]
#[rtype(result = "()")]
pub struct ClientAddNode {
    pub url: Url,
}

#[derive(Message)]
#[rtype(result = "()")]
pub struct ClientRemoveNode {
    pub url: Url,
}

#[derive(Message)]
#[rtype(result = "Vec<RouteStatus>")]
pub struct ClientListRoutes {}

#[derive(Message)]
#[rtype(result = "()")]
pub struct ClientAddRoute {
    pub target: Endpoint,
    pub next_hop: Endpoint,
}

#[derive(Message)]
#[rtype(result = "()")]
pub struct ClientRemoveRoute {
    pub target: Endpoint,
    pub next_hop: Endpoint,
}
