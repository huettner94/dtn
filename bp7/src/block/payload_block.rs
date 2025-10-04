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

use std::fmt::Debug;

use serde::Serialize;

use crate::Validate;

#[derive(PartialEq, Eq)]
pub struct PayloadBlock<'a> {
    pub data: &'a [u8],
}

impl<'a> Debug for PayloadBlock<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PayloadBlock")
            .field("data (length)", &self.data.len())
            .finish()
    }
}

impl<'a> Serialize for PayloadBlock<'a> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_bytes(self.data)
    }
}

impl<'a> Validate for PayloadBlock<'a> {
    fn validate(&self) -> bool {
        true
    }
}
