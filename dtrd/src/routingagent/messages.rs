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

use std::collections::HashMap;

use actix::prelude::*;
use bp7::endpoint::Endpoint;

#[derive(Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Clone, Copy)]
pub enum RouteType {
    Connected = 0,
    Static = 1,
}

#[derive(Debug, PartialEq, Eq)]
pub struct RouteStatus {
    pub target: Endpoint,
    pub next_hop: Endpoint,
    pub route_type: RouteType,
    pub preferred: bool,
    pub available: bool,
    pub max_bundle_size: Option<u64>,
}

#[derive(Debug, PartialEq, Eq)]
pub struct NexthopInfo {
    pub next_hop: Endpoint,
    pub max_size: Option<u64>,
}

#[derive(Message)]
#[rtype(result = "()")]
pub struct EventRoutingTableUpdate {
    pub routes: HashMap<Endpoint, NexthopInfo>,
}

#[derive(Message)]
#[rtype(result = "()")]
pub struct AddRoute {
    pub target: Endpoint,
    pub route_type: RouteType,
    pub next_hop: Endpoint,
    pub max_bundle_size: Option<u64>,
}

#[derive(Message)]
#[rtype(result = "()")]
pub struct RemoveRoute {
    pub target: Endpoint,
    pub route_type: RouteType,
    pub next_hop: Endpoint,
}

#[derive(Message)]
#[rtype(result = "Vec<RouteStatus>")]
pub struct ListRoutes {}
