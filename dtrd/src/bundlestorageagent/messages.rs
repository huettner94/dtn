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

use bp7::{bundle::Bundle, endpoint::Endpoint};

use super::StoredBundle;
use actix::prelude::*;

#[derive(Message)]
#[rtype(result = "")]
pub struct EventNewBundleStored {
    pub bundle: StoredBundle,
}

#[derive(Message)]
#[rtype(result = "Result<(), ()>")]
pub struct StoreBundle {
    pub bundle: Bundle,
}

#[derive(Message)]
#[rtype(result = "Result<(), ()>")]
pub struct StoreNewBundle {
    pub bundle: Bundle,
}

#[derive(Message)]
#[rtype(result = "")]
pub struct FragmentBundle {
    pub bundle: StoredBundle,
    pub target_size: u64,
}

#[derive(Message)]
#[rtype(result = "")]
pub struct DeleteBundle {
    pub bundle: StoredBundle,
}

#[derive(Message)]
#[rtype(result = "Result<Vec<StoredBundle>, String>")]
pub struct GetBundleForDestination {
    pub destination: Endpoint,
}

#[derive(Message)]
#[rtype(result = "Result<Vec<StoredBundle>, String>")]
pub struct GetBundleForNode {
    pub destination: Endpoint,
}
