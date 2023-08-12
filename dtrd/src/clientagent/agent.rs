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

use bp7::{
    block::{payload_block::PayloadBlock, Block, CanonicalBlock},
    blockflags::BlockFlags,
    bundle::Bundle,
    bundleflags::BundleFlags,
    crc::CRCType,
    endpoint::Endpoint,
    primaryblock::PrimaryBlock,
    time::{CreationTimestamp, DtnTime},
};
use log::{debug, info};
use tokio::sync::mpsc;

use crate::{
    bundlestorageagent::messages::StoreNewBundle,
    common::{messages::Shutdown, settings::Settings},
    nodeagent::messages::{AddNode, ListNodes, Node, RemoveNode},
    routingagent::messages::{AddRoute, ListRoutes, RemoveRoute, RouteStatus, RouteType},
};

use super::messages::{
    ClientAddNode, ClientAddRoute, ClientDeliverBundle, ClientListNodes, ClientListRoutes,
    ClientListenConnect, ClientListenDisconnect, ClientRemoveNode, ClientRemoveRoute,
    ClientSendBundle, EventBundleDeliveryFailed, EventClientConnected, EventClientDisconnected,
};
use actix::prelude::*;

#[derive(Default)]
pub struct Daemon {
    connected_clients: HashMap<Endpoint, Addr<ListenBundleResponseActor>>,
    endpoint: Option<Endpoint>,
}

impl Actor for Daemon {
    type Context = Context<Self>;
    fn started(&mut self, _ctx: &mut Context<Self>) {
        let settings = Settings::from_env();
        self.endpoint = Some(Endpoint::new(&settings.my_node_id).unwrap());
    }
}

impl actix::Supervised for Daemon {}

impl SystemService for Daemon {}

impl Handler<Shutdown> for Daemon {
    type Result = ();

    fn handle(&mut self, _msg: Shutdown, _ctx: &mut Self::Context) -> Self::Result {
        info!("Disconnecting all clients");
        for (_, client) in self.connected_clients.drain() {
            client.do_send(StopListenBundleResponseActor {});
        }
    }
}

impl Handler<ClientSendBundle> for Daemon {
    type Result = ResponseFuture<Result<(), ()>>;

    fn handle(&mut self, msg: ClientSendBundle, _ctx: &mut Context<Self>) -> Self::Result {
        let ClientSendBundle {
            destination,
            payload,
            lifetime,
        } = msg;

        let bundle = Bundle {
            primary_block: PrimaryBlock {
                version: 7,
                bundle_processing_flags: BundleFlags::BUNDLE_RECEIPTION_STATUS_REQUESTED
                    | BundleFlags::BUNDLE_FORWARDING_STATUS_REQUEST
                    | BundleFlags::BUNDLE_DELIVERY_STATUS_REQUESTED
                    | BundleFlags::BUNDLE_DELETION_STATUS_REQUESTED,
                crc: CRCType::NoCRC,
                destination_endpoint: destination,
                source_node: self.endpoint.as_ref().unwrap().clone(),
                report_to: self.endpoint.as_ref().unwrap().clone(),
                creation_timestamp: CreationTimestamp {
                    creation_time: DtnTime::now(),
                    sequence_number: 0,
                },
                lifetime,
                fragment_offset: None,
                total_data_length: None,
            },
            blocks: vec![CanonicalBlock {
                block: Block::Payload(PayloadBlock { data: payload }),
                block_flags: BlockFlags::empty(),
                block_number: 1,
                crc: CRCType::NoCRC,
            }],
        };
        debug!("Storing new bundle {:?}", &bundle.primary_block);
        Box::pin(async move {
            crate::bundlestorageagent::agent::Daemon::from_registry()
                .send(StoreNewBundle { bundle })
                .await
                .unwrap()
        })
    }
}

impl Handler<ClientListenConnect> for Daemon {
    type Result = Result<(), String>;

    fn handle(&mut self, msg: ClientListenConnect, _ctx: &mut Context<Self>) -> Self::Result {
        let ClientListenConnect {
            destination,
            sender,
        } = msg;

        if !self.endpoint.as_ref().unwrap().matches_node(&destination) {
            return Err("Listening endpoint does not match local node".to_string());
        }

        let response_actor = ListenBundleResponseActor {
            sender,
            endpoint: destination.clone(),
        };

        let response_actor_addr = response_actor.start();

        self.connected_clients
            .insert(destination.clone(), response_actor_addr.clone());

        crate::bundleprotocolagent::agent::Daemon::from_registry().do_send(EventClientConnected {
            destination,
            sender: response_actor_addr.recipient(),
        });

        Ok(())
    }
}

impl Handler<ClientListenDisconnect> for Daemon {
    type Result = ();

    fn handle(&mut self, msg: ClientListenDisconnect, _ctx: &mut Context<Self>) -> Self::Result {
        let ClientListenDisconnect { destination } = msg;

        if let Some(addr) = self.connected_clients.get(&destination) {
            addr.do_send(StopListenBundleResponseActor {});
        }

        crate::bundleprotocolagent::agent::Daemon::from_registry()
            .do_send(EventClientDisconnected { destination });
    }
}
impl Handler<ClientListNodes> for Daemon {
    type Result = ResponseFuture<Vec<Node>>;

    fn handle(&mut self, _msg: ClientListNodes, _ctx: &mut Context<Self>) -> Self::Result {
        Box::pin(async {
            crate::nodeagent::agent::Daemon::from_registry()
                .send(ListNodes {})
                .await
                .unwrap()
        })
    }
}

impl Handler<ClientAddNode> for Daemon {
    type Result = ();

    fn handle(&mut self, msg: ClientAddNode, _ctx: &mut Context<Self>) -> Self::Result {
        let ClientAddNode { url } = msg;
        crate::nodeagent::agent::Daemon::from_registry().do_send(AddNode { url });
    }
}

impl Handler<ClientRemoveNode> for Daemon {
    type Result = ();

    fn handle(&mut self, msg: ClientRemoveNode, _ctx: &mut Context<Self>) -> Self::Result {
        let ClientRemoveNode { url } = msg;
        crate::nodeagent::agent::Daemon::from_registry().do_send(RemoveNode { url });
    }
}

impl Handler<ClientListRoutes> for Daemon {
    type Result = ResponseFuture<Vec<RouteStatus>>;

    fn handle(&mut self, _msg: ClientListRoutes, _ctx: &mut Context<Self>) -> Self::Result {
        Box::pin(async {
            crate::routingagent::agent::Daemon::from_registry()
                .send(ListRoutes {})
                .await
                .unwrap()
        })
    }
}

impl Handler<ClientAddRoute> for Daemon {
    type Result = ();

    fn handle(&mut self, msg: ClientAddRoute, _ctx: &mut Context<Self>) -> Self::Result {
        let ClientAddRoute { target, next_hop } = msg;
        crate::routingagent::agent::Daemon::from_registry().do_send(AddRoute {
            target,
            next_hop,
            route_type: RouteType::Static,
            max_bundle_size: None,
        });
    }
}

impl Handler<ClientRemoveRoute> for Daemon {
    type Result = ();

    fn handle(&mut self, msg: ClientRemoveRoute, _ctx: &mut Context<Self>) -> Self::Result {
        let ClientRemoveRoute { target, next_hop } = msg;
        crate::routingagent::agent::Daemon::from_registry().do_send(RemoveRoute {
            target,
            next_hop,
            route_type: RouteType::Static,
        });
    }
}

#[derive(Message)]
#[rtype(result = "")]
struct StopListenBundleResponseActor {}

pub struct ListenBundleResponseActor {
    sender: mpsc::Sender<ClientDeliverBundle>,
    endpoint: Endpoint,
}

impl Actor for ListenBundleResponseActor {
    type Context = Context<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        ctx.set_mailbox_capacity(1);
    }
}

impl Handler<ClientDeliverBundle> for ListenBundleResponseActor {
    type Result = ();

    fn handle(&mut self, msg: ClientDeliverBundle, ctx: &mut Self::Context) -> Self::Result {
        let sender = self.sender.clone();
        let fut = async move { sender.send(msg).await };
        fut.into_actor(self)
            .then(|res, act, ctx| {
                match res {
                    Ok(_) => {}
                    Err(e) => {
                        crate::bundleprotocolagent::agent::Daemon::from_registry().do_send(
                            EventBundleDeliveryFailed {
                                bundle: e.0.bundle,
                                endpoint: act.endpoint.clone(),
                            },
                        );
                        ctx.stop();
                    }
                }
                fut::ready(())
            })
            .wait(ctx)
    }
}

impl Handler<StopListenBundleResponseActor> for ListenBundleResponseActor {
    type Result = ();

    fn handle(
        &mut self,
        _msg: StopListenBundleResponseActor,
        ctx: &mut Self::Context,
    ) -> Self::Result {
        ctx.stop()
    }
}
