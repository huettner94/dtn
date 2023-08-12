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

use bytes::BytesMut;

#[derive(Debug)]
pub struct Keepalive {}

impl Keepalive {
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        Keepalive {}
    }

    pub fn decode(_src: &mut BytesMut) -> Result<Option<Self>, crate::v4::messages::Errors> {
        Ok(Some(Keepalive {}))
    }

    pub fn encode(&self, _dst: &mut BytesMut) {}
}
