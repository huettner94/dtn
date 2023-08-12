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

use std::fmt::Display;

use actix::prelude::*;
use bp7::endpoint::Endpoint;
use url::Url;

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum NodeConnectionStatus {
    Disconnected,
    Connected,
    Connecting,
    Disconnecting,
}

impl Display for NodeConnectionStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            NodeConnectionStatus::Disconnected => "Disconnected",
            NodeConnectionStatus::Connected => "Connected",
            NodeConnectionStatus::Connecting => "Connecting",
            NodeConnectionStatus::Disconnecting => "Disconnecting",
        })
    }
}
#[derive(Debug, Clone, Eq)]
pub struct Node {
    pub url: Url,
    pub connection_status: NodeConnectionStatus,
    pub remote_endpoint: Option<Endpoint>,
    pub temporary: bool,
}

impl PartialEq for Node {
    fn eq(&self, other: &Self) -> bool {
        self.url == other.url
    }
}

#[derive(Message)]
#[rtype(result = "Vec<Node>")]
pub struct ListNodes {}

#[derive(Message)]
#[rtype(result = "")]
pub struct AddNode {
    pub url: Url,
}

#[derive(Message)]
#[rtype(result = "")]
pub struct RemoveNode {
    pub url: Url,
}

#[derive(Message)]
#[rtype(result = "")]
pub struct NotifyNodeConnected {
    pub url: Url,
    pub endpoint: Endpoint,
    pub max_bundle_size: u64,
}

#[derive(Message)]
#[rtype(result = "")]
pub struct NotifyNodeDisconnected {
    pub url: Url,
}

#[derive(Message)]
#[rtype(result = "")]
pub struct TryConnect {}
