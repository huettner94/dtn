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

use std::env;

#[derive(Debug, Clone)]
pub struct Settings {
    pub tokio_tracing_port: Option<String>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            tokio_tracing_port: None,
        }
    }
}

impl Settings {
    pub fn from_env() -> Self {
        let mut settings = Settings::default();
        if let Ok(setting) = env::var("TOKIO_TRACING_PORT") {
            settings.tokio_tracing_port = Some(setting);
        };
        settings
    }
}
