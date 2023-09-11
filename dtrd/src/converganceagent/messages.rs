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
use bp7::endpoint::Endpoint;
use url::Url;

use crate::bundlestorageagent::StoredBundle;

#[derive(Message)]
#[rtype(result = "()")]
pub struct AgentForwardBundle {
    pub bundle: StoredBundle,
    pub responder: Recipient<EventBundleForwarded>,
}

#[derive(Message)]
#[rtype(result = "()")]
pub struct EventBundleForwarded {
    pub endpoint: Endpoint,
    pub bundle: StoredBundle,
}

#[derive(Message)]
#[rtype(result = "()")]
pub struct EventBundleForwardingFailed {
    pub endpoint: Endpoint,
    pub bundle: StoredBundle,
}

#[derive(Message)]
#[rtype(result = "()")]
pub struct EventPeerConnected {
    pub destination: Endpoint,
    pub sender: Recipient<AgentForwardBundle>,
}

#[derive(Message)]
#[rtype(result = "()")]
pub struct EventPeerDisconnected {
    pub destination: Endpoint,
}

#[derive(Message)]
#[rtype(result = "()")]
pub struct AgentConnectNode {
    pub url: Url,
}
#[derive(Message)]
#[rtype(result = "()")]
pub struct AgentDisconnectNode {
    pub url: Url,
}
#[derive(Message)]
#[rtype(result = "()")]
pub struct CLRegisterNode {
    pub url: Url,
    pub node: Endpoint,
    pub max_bundle_size: u64,
    pub sender: Recipient<AgentForwardBundle>,
}
#[derive(Message)]
#[rtype(result = "()")]
pub struct CLUnregisterNode {
    pub url: Url,
    pub node: Option<Endpoint>,
}
