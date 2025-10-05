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
    pub my_node_id: String,
    pub tcpcl_listen_address: String,
    pub grpc_clientapi_address: String,
    pub tcpcl_certificate_path: Option<String>,
    pub tcpcl_key_path: Option<String>,
    pub tcpcl_trusted_certs_path: Option<String>,
    pub tokio_tracing_port: Option<String>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            my_node_id: "dtn://defaultnodeid".into(),
            tcpcl_listen_address: "[::1]:4556".into(),
            grpc_clientapi_address: "[::1]:50051".into(),
            tcpcl_certificate_path: None,
            tcpcl_key_path: None,
            tcpcl_trusted_certs_path: None,
            tokio_tracing_port: None,
        }
    }
}

impl Settings {
    pub fn from_env() -> Self {
        let mut settings = Settings::default();
        if let Ok(setting) = env::var("NODE_ID") {
            settings.my_node_id = setting;
        }
        if let Ok(setting) = env::var("TCPCL_LISTEN_ADDRESS") {
            settings.tcpcl_listen_address = setting;
        }
        if let Ok(setting) = env::var("GRPC_CLIENTAPI_ADDRESS") {
            settings.grpc_clientapi_address = setting;
        }
        if let Ok(setting) = env::var("TCPCL_CERTIFICATE_PATH") {
            settings.tcpcl_certificate_path = Some(setting);
        }
        if let Ok(setting) = env::var("TCPCL_KEY_PATH") {
            settings.tcpcl_key_path = Some(setting);
        }
        if let Ok(setting) = env::var("TCPCL_TRUSTED_CERTS_PATH") {
            settings.tcpcl_trusted_certs_path = Some(setting);
        }
        if let Ok(setting) = env::var("TOKIO_TRACING_PORT") {
            settings.tokio_tracing_port = Some(setting);
        }
        settings
    }
}
