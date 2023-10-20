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
use url::Url;

#[derive(Message)]
#[rtype(result = "()")]
pub struct ConnectRemote {
    pub url: Url,
}

#[derive(Message)]
#[rtype(result = "()")]
pub struct DisconnectRemote {
    pub url: Url,
}

#[derive(Message)]
#[rtype(result = "()")]
pub(crate) struct ForceShutdown {}
