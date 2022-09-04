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
use log::debug;

use crate::{
    bundlestorageagent::messages::StoreNewBundle,
    common::settings::Settings,
    nodeagent::messages::{AddNode, ListNodes, Node, RemoveNode},
    routingagent::messages::{AddRoute, ListRoutes, RemoveRoute, RouteStatus, RouteType},
};

use super::messages::{
    ClientAddNode, ClientAddRoute, ClientListNodes, ClientListRoutes, ClientRemoveNode,
    ClientRemoveRoute, ClientSendBundle,
};
use actix::prelude::*;

#[derive(Default)]
pub struct Daemon {
    //TODO clients: HashMap<Endpoint, (mpsc::Sender<ListenBundlesResponse>, CancellationToken)>,
    endpoint: Option<Endpoint>,
}

impl Actor for Daemon {
    type Context = Context<Self>;
    fn started(&mut self, ctx: &mut Context<Self>) {
        let settings = Settings::from_env();
        self.endpoint = Some(Endpoint::new(&settings.my_node_id).unwrap());
    }
}

impl actix::Supervised for Daemon {}

impl SystemService for Daemon {}

impl Handler<ClientSendBundle> for Daemon {
    type Result = ResponseFuture<Result<(), ()>>;

    fn handle(&mut self, msg: ClientSendBundle, ctx: &mut Context<Self>) -> Self::Result {
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

/* TODO impl Handler<ClientListenBundles> for Daemon {
    type Result = Result<(), String>;

    fn handle(&mut self, msg: ClientListenBundles, ctx: &mut Context<Self>) -> Self::Result {
        let ClientListenBundles {
            destination,
            responder,
            canceltoken,
        } = msg;
        let settings = Settings::from_env();
        let node_id = Endpoint::new(&settings.my_node_id).unwrap();

        if !node_id.matches_node(&destination) {
            warn!("User attempted to register with endpoint not bound here.");
            return Err("Endpoint invalid for this BundleProtocolAgent".to_string());
        }

        self.clients
            .insert(destination.clone(), (responder.clone(), canceltoken));
        crate::bundleprotocolagent::agent::Daemon::from_registry()
            .do_send(NewClientConnected { destination });

        Ok(())
    }
}*/

impl Handler<ClientListNodes> for Daemon {
    type Result = ResponseFuture<Vec<Node>>;

    fn handle(&mut self, msg: ClientListNodes, ctx: &mut Context<Self>) -> Self::Result {
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

    fn handle(&mut self, msg: ClientAddNode, ctx: &mut Context<Self>) -> Self::Result {
        let ClientAddNode { url } = msg;
        crate::nodeagent::agent::Daemon::from_registry().do_send(AddNode { url });
    }
}

impl Handler<ClientRemoveNode> for Daemon {
    type Result = ();

    fn handle(&mut self, msg: ClientRemoveNode, ctx: &mut Context<Self>) -> Self::Result {
        let ClientRemoveNode { url } = msg;
        crate::nodeagent::agent::Daemon::from_registry().do_send(RemoveNode { url });
    }
}

impl Handler<ClientListRoutes> for Daemon {
    type Result = ResponseFuture<Vec<RouteStatus>>;

    fn handle(&mut self, msg: ClientListRoutes, ctx: &mut Context<Self>) -> Self::Result {
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

    fn handle(&mut self, msg: ClientAddRoute, ctx: &mut Context<Self>) -> Self::Result {
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

    fn handle(&mut self, msg: ClientRemoveRoute, ctx: &mut Context<Self>) -> Self::Result {
        let ClientRemoveRoute { target, next_hop } = msg;
        crate::routingagent::agent::Daemon::from_registry().do_send(RemoveRoute {
            target,
            next_hop,
            route_type: RouteType::Static,
        });
    }
}
