use std::{
    collections::{HashMap, HashSet, VecDeque},
    mem,
};

use crate::{
    bundlestorageagent::{
        messages::{DeleteBundle, EventNewBundleStored, FragmentBundle, StoreNewBundle},
        StoredBundle,
    },
    clientagent::messages::{
        ClientDeliverBundle, EventBundleDelivered, EventBundleDeliveryFailed, EventClientConnected,
        EventClientDisconnected,
    },
    common::settings::Settings,
    converganceagent::messages::{
        AgentForwardBundle, EventBundleForwarded, EventBundleForwardingFailed, EventPeerConnected,
        EventPeerDisconnected,
    },
    routingagent::messages::{EventRoutingTableUpdate, NexthopInfo},
};
use bp7::{
    administrative_record::{
        bundle_status_report::{
            BundleStatusInformation, BundleStatusItem, BundleStatusReason, BundleStatusReport,
        },
        AdministrativeRecord,
    },
    block::payload_block::PayloadBlock,
    block::{Block, CanonicalBlock},
    blockflags::BlockFlags,
    bundle::Bundle,
    bundleflags::BundleFlags,
    crc::CRCType,
    endpoint::Endpoint,
    primaryblock::PrimaryBlock,
    time::{CreationTimestamp, DtnTime},
};
use log::{debug, warn};

use actix::prelude::*;

#[derive(Default)]
pub struct Daemon {
    endpoint: Option<Endpoint>,
    bundles_pending_local_delivery: HashMap<Endpoint, Vec<StoredBundle>>,
    bundles_pending_forwarding: HashMap<Endpoint, Vec<StoredBundle>>,
    local_bundles: HashMap<Endpoint, VecDeque<StoredBundle>>,
    remote_bundles: HashMap<Endpoint, VecDeque<StoredBundle>>,
    local_connections: HashMap<Endpoint, Recipient<ClientDeliverBundle>>,
    remote_connections: HashMap<Endpoint, Recipient<AgentForwardBundle>>,
    remote_routes: HashMap<Endpoint, NexthopInfo>,
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

impl Handler<EventNewBundleStored> for Daemon {
    type Result = ();

    fn handle(&mut self, msg: EventNewBundleStored, ctx: &mut Self::Context) -> Self::Result {
        let EventNewBundleStored { bundle } = msg;
        let destination = bundle
            .get_bundle()
            .primary_block
            .destination_endpoint
            .clone();
        if self.endpoint.as_ref().unwrap().matches_node(&destination) {
            self.local_bundles
                .entry(destination.clone())
                .or_default()
                .push_back(bundle);
            self.deliver_local_bundles(&destination, ctx)
        } else {
            self.send_status_report_received(bundle.get_bundle());
            self.remote_bundles
                .entry(destination.get_node_endpoint())
                .or_default()
                .push_back(bundle);
            self.deliver_remote_bundles(&destination, ctx);
        }
    }
}

impl Handler<EventBundleDelivered> for Daemon {
    type Result = ();

    fn handle(&mut self, msg: EventBundleDelivered, ctx: &mut Self::Context) -> Self::Result {
        let EventBundleDelivered { endpoint, bundle } = msg;
        if let Some(pending) = self.bundles_pending_local_delivery.get_mut(&endpoint) {
            pending.retain(|e| e != bundle)
        }
        self.send_status_report_delivered(bundle.get_bundle());

        crate::bundlestorageagent::agent::Daemon::from_registry().do_send(DeleteBundle { bundle });

        self.deliver_local_bundles(&endpoint, ctx);
    }
}

impl Handler<EventBundleDeliveryFailed> for Daemon {
    type Result = ();

    fn handle(&mut self, msg: EventBundleDeliveryFailed, _ctx: &mut Self::Context) -> Self::Result {
        let EventBundleDeliveryFailed { endpoint, bundle } = msg;
        warn!(
            "Delivering local bundle to endpoint {} failed. Requeueing",
            &endpoint
        );
        if let Some(pending) = self.bundles_pending_local_delivery.get_mut(&endpoint) {
            pending.retain(|e| e != bundle)
        }
        self.local_bundles
            .entry(endpoint)
            .or_default()
            .push_front(bundle);
    }
}

impl Handler<EventClientConnected> for Daemon {
    type Result = ();

    fn handle(&mut self, msg: EventClientConnected, ctx: &mut Context<Self>) {
        let EventClientConnected {
            destination,
            sender,
        } = msg;

        self.local_connections
            .insert(destination.clone(), sender.clone());

        self.deliver_local_bundles(&destination, ctx);
    }
}

impl Handler<EventClientDisconnected> for Daemon {
    type Result = ();

    fn handle(&mut self, msg: EventClientDisconnected, _ctx: &mut Self::Context) -> Self::Result {
        let EventClientDisconnected { destination } = msg;
        self.local_connections.remove(&destination);
    }
}

impl Handler<EventPeerConnected> for Daemon {
    type Result = ();

    fn handle(&mut self, msg: EventPeerConnected, ctx: &mut Context<Self>) {
        let EventPeerConnected {
            destination,
            sender,
        } = msg;
        assert!(destination.get_node_endpoint() == destination);

        self.remote_connections
            .insert(destination.clone(), sender.clone());

        self.deliver_remote_bundles(&destination, ctx);
    }
}

impl Handler<EventPeerDisconnected> for Daemon {
    type Result = ();

    fn handle(&mut self, msg: EventPeerDisconnected, _ctx: &mut Self::Context) -> Self::Result {
        let EventPeerDisconnected { destination } = msg;
        assert!(destination.get_node_endpoint() == destination);
        self.remote_connections.remove(&destination);
    }
}

impl Handler<EventBundleForwarded> for Daemon {
    type Result = ();

    fn handle(&mut self, msg: EventBundleForwarded, ctx: &mut Self::Context) -> Self::Result {
        let EventBundleForwarded { endpoint, bundle } = msg;
        let endpoint = endpoint.get_node_endpoint();
        if let Some(pending) = self.bundles_pending_forwarding.get_mut(&endpoint) {
            pending.retain(|e| e != bundle)
        }
        self.send_status_report_forwarded(bundle.get_bundle());

        crate::bundlestorageagent::agent::Daemon::from_registry().do_send(DeleteBundle { bundle });

        self.deliver_remote_bundles(&endpoint, ctx);
    }
}

impl Handler<EventBundleForwardingFailed> for Daemon {
    type Result = ();

    fn handle(
        &mut self,
        msg: EventBundleForwardingFailed,
        _ctx: &mut Self::Context,
    ) -> Self::Result {
        let EventBundleForwardingFailed { endpoint, bundle } = msg;
        let endpoint = endpoint.get_node_endpoint();
        warn!(
            "Forwarding bundle to endpoint {} failed. Requeueing",
            &endpoint
        );
        if let Some(pending) = self.bundles_pending_forwarding.get_mut(&endpoint) {
            pending.retain(|e| e != bundle)
        }
        self.remote_bundles
            .entry(endpoint)
            .or_default()
            .push_front(bundle);
    }
}

impl Handler<EventRoutingTableUpdate> for Daemon {
    type Result = ();

    fn handle(&mut self, msg: EventRoutingTableUpdate, ctx: &mut Self::Context) -> Self::Result {
        debug!("Updating routing table");
        let old_routes = mem::replace(&mut self.remote_routes, msg.routes);
        let old_route_targets: HashSet<Endpoint> = old_routes.keys().map(|k| k.clone()).collect();
        let new_route_targets: HashSet<Endpoint> =
            self.remote_routes.keys().map(|k| k.clone()).collect();
        let added_routes: HashSet<&Endpoint> =
            new_route_targets.difference(&old_route_targets).collect();
        let updated_routes: HashSet<&Endpoint> =
            new_route_targets.intersection(&old_route_targets).collect();
        for added_route in added_routes {
            debug!("New route available for {}", added_route);
            self.deliver_remote_bundles(added_route, ctx);
        }
        for updated_route in updated_routes {
            debug!("Updated route available for {}", updated_route);
            self.deliver_remote_bundles(updated_route, ctx);
        }
    }
}

impl Daemon {
    fn deliver_local_bundles(&mut self, destination: &Endpoint, ctx: &mut Context<Self>) {
        let sender = match self.local_connections.get(destination) {
            Some(s) => s,
            None => return,
        };

        match self.local_bundles.get_mut(&destination) {
            Some(queue) => {
                while let Some(bundle) = queue.pop_front() {
                    debug!(
                        "locally delivering bundle {:?}",
                        &bundle.get_bundle().primary_block
                    );
                    if bundle.get_bundle().primary_block.fragment_offset.is_some() {
                        panic!("Bundle is a fragment. It should have been reassembled before calling this");
                    }

                    match sender.try_send(ClientDeliverBundle {
                        bundle: bundle.clone(),
                        responder: ctx.address().recipient(),
                    }) {
                        Ok(_) => self
                            .bundles_pending_local_delivery
                            .entry(destination.clone())
                            .or_default()
                            .push(bundle),
                        Err(e) => match e {
                            SendError::Full(cdb) => {
                                let ClientDeliverBundle { bundle, .. } = cdb;
                                queue.push_back(bundle);
                                return;
                            }
                            SendError::Closed(_) => {
                                warn!("Client for endpoint {} disconnected while sending bundles. Queueing...", destination);
                                queue.push_back(bundle);
                                self.local_connections.remove(&destination);
                                return;
                            }
                        },
                    }
                }
            }
            None => {}
        }
    }

    fn deliver_remote_bundles(&mut self, destination: &Endpoint, ctx: &mut Context<Self>) {
        let destination = destination.get_node_endpoint();
        let route = match self.remote_routes.get(&destination) {
            Some(n) => n,
            None => return,
        };
        let sender = match self.remote_connections.get(&route.next_hop) {
            Some(s) => s,
            None => return,
        };
        let sender_route = match self.remote_routes.get(&route.next_hop) {
            Some(n) => n,
            None => return,
        };
        // This gets the smaller max_bundle_size for both of them, ignoring any Nones
        let max_bundle_size = match route.max_size {
            Some(ms) => Some(match sender_route.max_size {
                Some(s_ms) => ms.min(s_ms),
                None => ms,
            }),
            None => sender_route.max_size,
        };

        match self.remote_bundles.get_mut(&destination) {
            Some(queue) => {
                let mut visited: HashSet<uuid::Uuid> = HashSet::new();
                while let Some(bundle) = queue.pop_front() {
                    if visited.contains(&bundle.get_id()) {
                        break;
                    }
                    debug!(
                        "forwarding bundle {:?} to {:?}",
                        &bundle.get_bundle().primary_block,
                        destination
                    );

                    if max_bundle_size.is_some()
                        && bundle.get_bundle_size() > max_bundle_size.unwrap()
                    {
                        if bundle
                            .get_bundle()
                            .primary_block
                            .bundle_processing_flags
                            .contains(BundleFlags::MUST_NOT_FRAGMENT)
                            || bundle.get_bundle_min_size().is_some()
                                && bundle.get_bundle_min_size().unwrap() > max_bundle_size.unwrap()
                        {
                            debug!("Bundle can not be fragmented as we can not get it that small. Queueing it");
                            visited.insert(bundle.get_id());
                            queue.push_back(bundle);
                        } else {
                            crate::bundlestorageagent::agent::Daemon::from_registry().do_send(
                                FragmentBundle {
                                    bundle,
                                    target_size: max_bundle_size.unwrap(),
                                },
                            );
                        }
                        continue;
                    }

                    match sender.try_send(AgentForwardBundle {
                        bundle: bundle.clone(),
                        responder: ctx.address().recipient(),
                    }) {
                        Ok(_) => self
                            .bundles_pending_forwarding
                            .entry(destination.clone())
                            .or_default()
                            .push(bundle),
                        Err(e) => match e {
                            SendError::Full(afb) => {
                                debug!("Can not continue forwarding to {}. Waiting for some space in the queue", route.next_hop);
                                let AgentForwardBundle { bundle, .. } = afb;
                                queue.push_back(bundle);
                                return;
                            }
                            SendError::Closed(_) => {
                                warn!("Peer for endpoint {} disconnected while forwarding bundles. Queueing...", destination);
                                queue.push_back(bundle);
                                self.remote_connections.remove(&destination);
                                return;
                            }
                        },
                    }
                }
            }
            None => {}
        }
    }

    fn send_status_report(
        &mut self,
        bundle: &Bundle,
        reason: BundleStatusReason,
        is_received: bool,
        is_forwarded: bool,
        is_delivered: bool,
        is_deleted: bool,
    ) {
        let now = DtnTime::now();
        let received_info = BundleStatusItem {
            is_asserted: is_received,
            timestamp: if is_received { Some(now) } else { None },
        };
        let forwarded_info = BundleStatusItem {
            is_asserted: is_forwarded,
            timestamp: if is_forwarded { Some(now) } else { None },
        };
        let delivered_info = BundleStatusItem {
            is_asserted: is_delivered,
            timestamp: if is_delivered { Some(now) } else { None },
        };
        let deleted_info = BundleStatusItem {
            is_asserted: is_deleted,
            timestamp: if is_deleted { Some(now) } else { None },
        };
        match AdministrativeRecord::BundleStatusReport(BundleStatusReport {
            status_information: BundleStatusInformation {
                received_bundle: received_info,
                forwarded_bundle: forwarded_info,
                delivered_bundle: delivered_info,
                deleted_bundle: deleted_info,
            },
            reason,
            bundle_source: bundle.primary_block.source_node.clone(),
            bundle_creation_timestamp: bundle.primary_block.creation_timestamp.clone(),
            fragment_offset: bundle.primary_block.fragment_offset,
            fragment_length: bundle.primary_block.total_data_length,
        })
        .try_into()
        {
            Ok(data) => {
                let bundle = Bundle {
                    primary_block: PrimaryBlock {
                        version: 7,
                        bundle_processing_flags: BundleFlags::ADMINISTRATIVE_RECORD,
                        crc: CRCType::NoCRC,
                        destination_endpoint: bundle.primary_block.report_to.clone(),
                        source_node: self.endpoint.as_ref().unwrap().clone(),
                        report_to: self.endpoint.as_ref().unwrap().clone(),
                        creation_timestamp: CreationTimestamp {
                            creation_time: DtnTime::now(),
                            sequence_number: 0, // TODO: Needs to increase for all of the same timestamp
                        },
                        lifetime: bundle.primary_block.lifetime,
                        fragment_offset: None,
                        total_data_length: None,
                    },
                    blocks: vec![CanonicalBlock {
                        block: Block::Payload(PayloadBlock { data }),
                        block_flags: BlockFlags::empty(),
                        block_number: 1,
                        crc: CRCType::NoCRC,
                    }],
                };
                debug!(
                    "Dispatching administrative record bundle {:?}",
                    &bundle.primary_block
                );
                crate::bundlestorageagent::agent::Daemon::from_registry()
                    .do_send(StoreNewBundle { bundle });
            }
            Err(e) => {
                warn!("Error serializing bundle status report: {:?}", e)
            }
        };
    }

    fn send_status_report_delivered(&mut self, bundle: &Bundle) {
        if !bundle
            .primary_block
            .bundle_processing_flags
            .contains(BundleFlags::BUNDLE_DELIVERY_STATUS_REQUESTED)
        {
            return;
        }
        self.send_status_report(
            bundle,
            BundleStatusReason::NoAdditionalInformation,
            false,
            false,
            true,
            false,
        );
    }

    fn send_status_report_forwarded(&mut self, bundle: &Bundle) {
        if !bundle
            .primary_block
            .bundle_processing_flags
            .contains(BundleFlags::BUNDLE_FORWARDING_STATUS_REQUEST)
        {
            return;
        }
        self.send_status_report(
            bundle,
            BundleStatusReason::NoAdditionalInformation,
            false,
            true,
            false,
            false,
        );
    }

    fn send_status_report_received(&mut self, bundle: &Bundle) {
        if !bundle
            .primary_block
            .bundle_processing_flags
            .contains(BundleFlags::BUNDLE_RECEIPTION_STATUS_REQUESTED)
        {
            return;
        }
        self.send_status_report(
            bundle,
            BundleStatusReason::NoAdditionalInformation,
            true,
            false,
            false,
            false,
        );
    }
}
