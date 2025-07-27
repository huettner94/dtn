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

use std::net::SocketAddr;

use bp7::endpoint::Endpoint;
use log::{debug, error, warn};
use tcpcl::{
    connection_info::ConnectionInfo, errors::TransferSendErrors, session::TCPCLSession,
    transfer::Transfer,
};
use tokio::{
    net::TcpStream,
    sync::{mpsc, oneshot},
};
use tokio_stream::wrappers::ReceiverStream;

use crate::{
    bundlestorageagent::messages::StoreBundle,
    common::messages::Shutdown,
    converganceagent::messages::{
        AgentForwardBundle, CLRegisterNode, CLUnregisterNode, EventBundleForwarded,
        EventBundleForwardingFailed,
    },
};

use actix::prelude::*;

use super::messages::ForceShutdown;

#[derive(Message)]
#[rtype(result = "()")]
pub struct NewClientConnectedOnSocket {
    pub stream: TcpStream,
    pub address: SocketAddr,
}

type TCPCLSendChannel = mpsc::Sender<(Vec<u8>, oneshot::Sender<Result<(), TransferSendErrors>>)>;

pub struct TCPCLSessionAgent {
    close_channel: Option<oneshot::Sender<()>>,
    send_channel: TCPCLSendChannel,
}

impl Actor for TCPCLSessionAgent {
    type Context = Context<Self>;

    fn stopping(&mut self, _ctx: &mut Self::Context) -> Running {
        // This isn't what it looks like :)
        // when we finish establishing the tcpcl session we get the information
        // about the connection using StreamHandler<ConnectionInfo>. However
        // the stream is closed afterwards. Actix treats this as a reason to stop
        // the whole actor. With this pice of code we only stop when we actually
        // want to or if the connection got finished by the remote (send_channel)
        if self.close_channel.is_some() && !self.send_channel.is_closed() {
            Running::Continue
        } else {
            Running::Stop
        }
    }
}

impl StreamHandler<Transfer> for TCPCLSessionAgent {
    fn handle(&mut self, item: Transfer, ctx: &mut Self::Context) {
        match item.data.try_into() {
            Ok(bundle) => {
                let transferid = item.id;
                crate::bundlestorageagent::agent::Daemon::from_registry()
                    .send(StoreBundle { bundle })
                    .into_actor(self)
                    .then(move |res, _act, _ctx| {
                        match res.unwrap() {
                            Ok(_) => {debug!("Successfully received transfer {}", transferid)},
                            Err(_) => {error!("Error storing transfered bundle with id {}. It would have been better if we did not ack it", transferid)},
                        };
                        fut::ready(())})
                    .spawn(ctx);
            }
            Err(e) => {
                error!("Error deserializing bundle from remote: {:?}", e);
            }
        };
    }
}

impl Handler<AgentForwardBundle> for TCPCLSessionAgent {
    type Result = ();

    fn handle(&mut self, msg: AgentForwardBundle, ctx: &mut Self::Context) -> Self::Result {
        let AgentForwardBundle { bundle, responder } = msg;
        let (result_sender, result_receiver) = oneshot::channel();

        let bundle_data = match bundle.get_bundle().try_into() {
            Ok(bundle_data) => bundle_data,
            Err(e) => {
                error!("Error serializing bundle: {:?}", e);
                return;
            }
        };
        let bundle_endpoint = bundle
            .get_bundle()
            .primary_block
            .destination_endpoint
            .clone();

        let channel = self.send_channel.clone();
        let fut = async move { channel.send((bundle_data, result_sender)).await };
        fut.into_actor(self)
            .then(|res, _act, ctx| {
                if res.is_err() {
                    error!("Error sending bundle to tcpcl connection. Killing the connection");
                    ctx.stop();
                } else {
                    let listener = async move {
                        match result_receiver.await {
                            Ok(send_result) => match send_result {
                                Ok(_) => {
                                    responder
                                        .send(EventBundleForwarded {
                                            endpoint: bundle_endpoint,
                                            bundle,
                                        })
                                        .await
                                        .unwrap();
                                }
                                Err(e) => {
                                    error!("Error during sending of bundle: {:?}", e);
                                    crate::bundleprotocolagent::agent::Daemon::from_registry()
                                        .send(EventBundleForwardingFailed {
                                            endpoint: bundle_endpoint,
                                            bundle,
                                        })
                                        .await
                                        .unwrap();
                                }
                            },
                            Err(_) => {
                                debug!("Error during receiving bundle status results. Probabily the session was killed ugly");
                            }
                        }
                    };
                    tokio::spawn(listener); // We drop the join handle here because we never need to access it again
                }
                fut::ready(())
            })
            .wait(ctx);
    }
}

impl Handler<Shutdown> for TCPCLSessionAgent {
    type Result = ();

    fn handle(&mut self, _msg: Shutdown, ctx: &mut Self::Context) -> Self::Result {
        if let Some(c) = self.close_channel.take() {
            if c.send(()).is_err() {
                warn!(
                    "Error sending shutdown message to tcpcl session. Forcing it to die by stopping us"
                );
                ctx.stop();
            }
        }
    }
}

impl Handler<ForceShutdown> for TCPCLSessionAgent {
    type Result = ();

    fn handle(&mut self, _msg: ForceShutdown, ctx: &mut Self::Context) -> Self::Result {
        ctx.stop();
    }
}

impl StreamHandler<ConnectionInfo> for TCPCLSessionAgent {
    fn handle(&mut self, item: ConnectionInfo, ctx: &mut Self::Context) {
        match Endpoint::new(item.peer_endpoint.as_ref().unwrap()) {
            Some(node) => {
                crate::converganceagent::agent::Daemon::from_registry().do_send(CLRegisterNode {
                    url: item.peer_url,
                    node,
                    max_bundle_size: item
                        .max_bundle_size
                        .expect("We must have a bundle size if we are connected"),
                    sender: ctx.address().recipient(),
                });
            }
            None => {
                warn!(
                    "Peer send invalid id '{}'.",
                    item.peer_endpoint.as_ref().unwrap()
                );
                ctx.stop();
            }
        }
    }
}

impl TCPCLSessionAgent {
    pub fn new(mut session: TCPCLSession) -> Addr<Self> {
        TCPCLSessionAgent::create(|ctx| {
            ctx.add_stream(ReceiverStream::new(session.get_receive_channel()));

            let established_channel = session.get_established_channel();
            ctx.add_stream(async_stream::stream! {yield established_channel.await.unwrap();});

            let close_channel = session.get_close_channel();
            let send_channel = session.get_send_channel();

            let session_agent_address = ctx.address();

            let fut = async move {
                if let Err(e) = session.manage_connection().await {
                    warn!("Connection closed with error: {:?}", e);
                    session_agent_address.do_send(ForceShutdown {});
                }
                let ci = session.get_connection_info();
                let node = match ci.peer_endpoint {
                    Some(endpoint) => Endpoint::new(&endpoint),
                    None => None,
                };
                crate::converganceagent::agent::Daemon::from_registry().do_send(CLUnregisterNode {
                    url: ci.peer_url,
                    node,
                });
            };
            tokio::spawn(fut); // We drop the join handle here because we never need to access it again

            TCPCLSessionAgent {
                close_channel: Some(close_channel),
                send_channel,
            }
        })
    }
}
