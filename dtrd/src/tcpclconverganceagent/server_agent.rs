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

use std::{collections::HashMap, io, net::SocketAddr};

use log::{error, info};
use openssl::{pkey::PKey, x509::X509};
use tcpcl::{session::TCPCLSession, TLSSettings};
use tokio::{
    fs::File,
    io::AsyncReadExt,
    net::TcpListener,
    sync::{broadcast, mpsc},
    task::JoinHandle,
};
use url::Url;

use crate::{
    common::{messages::Shutdown, settings::Settings},
    converganceagent::messages::CLUnregisterNode,
    tcpclconverganceagent::session_agent::NewClientConnectedOnSocket,
};

use actix::{prelude::*, spawn};

use super::{
    messages::{ConnectRemote, DisconnectRemote},
    session_agent::TCPCLSessionAgent,
};

pub async fn tcpcl_listener(
    mut shutdown: broadcast::Receiver<()>,
    _shutdown_complete_sender: mpsc::Sender<()>,
    tcpcl_server: Addr<TCPCLServer>,
) -> Result<JoinHandle<()>, io::Error> {
    let settings = Settings::from_env();

    let socket: SocketAddr = settings.tcpcl_listen_address.parse().unwrap();

    info!("Server listening on {}", socket);

    let listener = TcpListener::bind(&socket).await?;

    let joinhandle = spawn(async move {
        info!("Socket open, waiting for connection");
        loop {
            tokio::select! {
                conn = listener.accept() => {
                    match conn {
                        Ok((stream, address)) => {
                            tcpcl_server.do_send(NewClientConnectedOnSocket {stream, address});
                        },
                        Err(e) => {
                            error!("Something bad happend during accepting a connection for tcpcl: {:?}. Aborting...", &e);
                        }
                    }
                }
                _ = shutdown.recv() => {
                    info!("Received shutdown message, stopping the tcpcl socket");
                    break;
                }
            };
        }

        drop(listener); // implicitly closes the socket

        info!("TCPCL socket has shutdown. See you");
        // _shutdown_complete_sender is implicitly dropped here
    });
    Ok(joinhandle)
}

#[derive(Default)]
pub struct TCPCLServer {
    my_node_id: String,
    tls_config: Option<TLSSettings>,
    sessions: HashMap<Url, Addr<TCPCLSessionAgent>>,
}

impl Actor for TCPCLServer {
    type Context = Context<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        let settings = Settings::from_env();
        self.my_node_id = settings.my_node_id.clone();

        let fut = async move { TCPCLServer::load_tls_settings(&settings).await };
        fut.into_actor(self)
            .then(|res, act, ctx| {
                match res {
                    Ok(tls_config) => act.tls_config = tls_config,
                    Err(e) => {
                        error!("Error loading TLS configuration for tcpcl server: {}", e);
                        ctx.stop();
                    }
                }
                fut::ready(())
            })
            .wait(ctx);
    }
}

impl actix::Supervised for TCPCLServer {}

impl SystemService for TCPCLServer {}

impl Handler<NewClientConnectedOnSocket> for TCPCLServer {
    type Result = ();

    fn handle(
        &mut self,
        msg: NewClientConnectedOnSocket,
        _ctx: &mut Self::Context,
    ) -> Self::Result {
        let NewClientConnectedOnSocket { stream, address } = msg;
        info!("New client connected from {}", address);
        let session =
            match TCPCLSession::new(stream, self.my_node_id.clone(), self.tls_config.clone()) {
                Ok(s) => s,
                Err(e) => {
                    error!(
                        "Error handling new incoming connection: {:?}. Connection will be dropped",
                        e
                    );
                    return;
                }
            };

        let sessionagent = TCPCLSessionAgent::new(session);
        let url = Url::parse(&format!("tcpcl://{}", address)).unwrap();
        self.sessions.insert(url, sessionagent);
    }
}

impl Handler<ConnectRemote> for TCPCLServer {
    type Result = ();

    fn handle(&mut self, msg: ConnectRemote, ctx: &mut Self::Context) -> Self::Result {
        let ConnectRemote { url } = msg;

        let fut = TCPCLSession::connect(
            url.clone(),
            self.my_node_id.clone(),
            self.tls_config.clone(),
        );
        fut.into_actor(self)
            .then(move |ret, act, _ctx| {
                match ret {
                    Ok(session) => {
                        let sessionagent = TCPCLSessionAgent::new(session);
                        act.sessions.insert(url, sessionagent);
                    }
                    Err(e) => {
                        error!("Error connecting to remote tcpcl: {:?}", e);
                        crate::converganceagent::agent::Daemon::from_registry()
                            .do_send(CLUnregisterNode { url, node: None });
                    }
                }
                fut::ready(())
            })
            .wait(ctx);
    }
}

impl Handler<DisconnectRemote> for TCPCLServer {
    type Result = ();

    fn handle(&mut self, msg: DisconnectRemote, _ctx: &mut Self::Context) -> Self::Result {
        let DisconnectRemote { url } = msg;
        if let Some(sess) = self.sessions.remove(&url) {
            sess.do_send(Shutdown {});
        }
    }
}

impl Handler<Shutdown> for TCPCLServer {
    type Result = ();

    fn handle(&mut self, _msg: Shutdown, _ctx: &mut Self::Context) -> Self::Result {
        for (_, session) in self.sessions.drain() {
            session.do_send(Shutdown {});
        }
    }
}

impl TCPCLServer {
    async fn load_tls_settings(settings: &Settings) -> Result<Option<TLSSettings>, std::io::Error> {
        if settings.tcpcl_certificate_path.is_some()
            && settings.tcpcl_key_path.is_some()
            && settings.tcpcl_trusted_certs_path.is_some()
        {
            let mut certificate_file =
                File::open(settings.tcpcl_certificate_path.as_ref().unwrap()).await?;
            let mut certificate_data = Vec::new();
            certificate_file.read_to_end(&mut certificate_data).await?;
            let certificate = if certificate_data.starts_with(b"-----BEGIN CERTIFICATE-----") {
                X509::from_pem(&certificate_data)?
            } else {
                X509::from_der(&certificate_data)?
            };

            let mut key_file = File::open(settings.tcpcl_key_path.as_ref().unwrap()).await?;
            let mut key_data = Vec::new();
            key_file.read_to_end(&mut key_data).await?;
            let key = if key_data.starts_with(b"-----BEGIN RSA PRIVATE KEY-----") {
                PKey::private_key_from_pem(&key_data)?
            } else {
                PKey::private_key_from_der(&key_data)?
            };

            let mut trusted_file =
                File::open(settings.tcpcl_trusted_certs_path.as_ref().unwrap()).await?;
            let mut trusted_data = Vec::new();
            trusted_file.read_to_end(&mut trusted_data).await?;
            let trusted = if trusted_data.starts_with(b"-----BEGIN CERTIFICATE-----") {
                X509::from_pem(&trusted_data)?
            } else {
                X509::from_der(&trusted_data)?
            };
            info!("Starting TCPCL agent with TLS Support");
            return Ok(Some(TLSSettings::new(key, certificate, vec![trusted])));
        }
        info!("Starting TCPCL agent without TLS Support");
        Ok(None)
    }
}
