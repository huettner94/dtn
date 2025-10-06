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

use bp7::endpoint::Endpoint;

use crate::bundlestorageagent::{State, StoredBundleRef};

use actix::prelude::*;

#[derive(Message)]
#[rtype(result = "()")]
pub struct EventNewBundleStored {
    pub bundle: StoredBundleRef,
}

#[derive(Message)]
#[rtype(result = "()")]
pub struct EventBundleUpdated {
    pub bundle: StoredBundleRef,
}

#[derive(Message)]
#[rtype(result = "()")]
pub struct StoreBundle {
    pub bundle_data: Vec<u8>,
}

#[derive(Message)]
#[rtype(result = "Result<(), ()>")]
pub struct StoreNewBundle {
    pub bundle_data: Vec<u8>,
}

#[derive(Message)]
#[rtype(result = "()")]
pub struct UpdateBundle {
    pub bundleref: StoredBundleRef,
    pub new_state: State,
    pub new_data: Option<Vec<u8>>,
}

#[derive(Message)]
#[rtype(result = "()")]
pub struct FragmentBundle {
    pub bundleref: StoredBundleRef,
    pub target_size: u64,
}

#[derive(Message)]
#[rtype(result = "Result<Vec<StoredBundleRef>, String>")]
pub struct GetBundleForDestination {
    pub destination: Endpoint,
}

#[derive(Message)]
#[rtype(result = "Result<Vec<StoredBundleRef>, String>")]
pub struct GetBundleForNode {
    pub destination: Endpoint,
}
