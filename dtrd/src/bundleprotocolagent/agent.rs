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

use std::{
    collections::{HashMap, HashSet, VecDeque},
    mem,
};

use crate::{
    bundlestorageagent::{
        State, StoredBundleRef,
        messages::{
            EventBundleUpdated, EventNewBundleStored, FragmentBundle, StoreNewBundle, UpdateBundle,
        },
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
        AdministrativeRecord,
        bundle_status_report::{
            BundleStatusInformation, BundleStatusItem, BundleStatusReason, BundleStatusReport,
        },
    },
    block::{Block, CanonicalBlock, payload_block::PayloadBlock},
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

const HOP_LIMIT_DEFAULT: u8 = 16;

#[derive(Default)]
pub struct Daemon {
    endpoint: Option<Endpoint>,
    bundles_pending_local_delivery: HashMap<Endpoint, Vec<StoredBundleRef>>,
    bundles_pending_forwarding: HashMap<Endpoint, Vec<StoredBundleRef>>,
    local_bundles: HashMap<Endpoint, VecDeque<StoredBundleRef>>,
    remote_bundles: HashMap<Endpoint, VecDeque<StoredBundleRef>>,
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

    fn handle(&mut self, msg: EventNewBundleStored, _ctx: &mut Self::Context) -> Self::Result {
        let EventNewBundleStored { bundle } = msg;
        // TODO: validation
        if !bundle
            .get_primary_block()
            .source_node
            .matches_node(self.endpoint.as_ref().unwrap())
        {
            self.send_status_report_received(&bundle);
        }
        crate::bundlestorageagent::agent::Daemon::from_registry().do_send(UpdateBundle {
            bundleref: bundle,
            new_state: State::Valid,
            new_data: None,
        });
    }
}

impl Handler<EventBundleUpdated> for Daemon {
    type Result = ();

    fn handle(&mut self, msg: EventBundleUpdated, ctx: &mut Self::Context) -> Self::Result {
        let EventBundleUpdated { bundle } = msg;
        let destination = bundle.get_primary_block().destination_endpoint.clone();
        match bundle.get_state() {
            State::Received => unreachable!(),
            State::Valid => {
                if self.endpoint.as_ref().unwrap().matches_node(&destination) {
                    crate::bundlestorageagent::agent::Daemon::from_registry().do_send(
                        UpdateBundle {
                            bundleref: bundle,
                            new_state: State::DeliveryQueued,
                            new_data: None,
                        },
                    );
                } else {
                    match self.forward_bundle(&bundle) {
                        Ok(new_data) => {
                            crate::bundlestorageagent::agent::Daemon::from_registry().do_send(
                                UpdateBundle {
                                    bundleref: bundle,
                                    new_state: State::ForwardingQueued,
                                    new_data: Some(new_data),
                                },
                            );
                        }
                        Err(e) => {
                            warn!("forwarding bundle failed: {e:?}");
                            self.send_status_report_deleted(&bundle, e);
                            crate::bundlestorageagent::agent::Daemon::from_registry().do_send(
                                UpdateBundle {
                                    bundleref: bundle,
                                    new_state: State::Invalid,
                                    new_data: None,
                                },
                            );
                        }
                    }
                }
            }
            State::DeliveryQueued => {
                self.local_bundles
                    .entry(destination.clone())
                    .or_default()
                    .push_back(bundle);
                self.deliver_local_bundles(&destination, ctx);
            }
            State::ForwardingQueued => {
                self.remote_bundles
                    .entry(destination.get_node_endpoint())
                    .or_default()
                    .push_back(bundle);
                self.deliver_remote_bundles(&destination, ctx);
            }
            State::Delivered | State::Forwarded | State::Invalid => {
                // Ignoring, all is done from our side
            }
        }
    }
}

impl Handler<EventBundleDelivered> for Daemon {
    type Result = ();

    fn handle(&mut self, msg: EventBundleDelivered, ctx: &mut Self::Context) -> Self::Result {
        let EventBundleDelivered { endpoint, bundle } = msg;
        if let Some(pending) = self.bundles_pending_local_delivery.get_mut(&endpoint) {
            pending.retain(|e| e != bundle);
        }
        self.send_status_report_delivered(&bundle);

        crate::bundlestorageagent::agent::Daemon::from_registry().do_send(UpdateBundle {
            bundleref: bundle,
            new_state: State::Delivered,
            new_data: None,
        });

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
            pending.retain(|e| e != bundle);
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

        self.local_connections.insert(destination.clone(), sender);

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

        self.remote_connections.insert(destination.clone(), sender);

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
            pending.retain(|e| e != bundle);
        }
        self.send_status_report_forwarded(&bundle);

        crate::bundlestorageagent::agent::Daemon::from_registry().do_send(UpdateBundle {
            bundleref: bundle,
            new_state: State::Forwarded,
            new_data: None,
        });

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
            pending.retain(|e| e != bundle);
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
        let old_route_targets: HashSet<Endpoint> = old_routes.keys().cloned().collect();
        let new_route_targets: HashSet<Endpoint> = self.remote_routes.keys().cloned().collect();
        let added_routes: HashSet<&Endpoint> =
            new_route_targets.difference(&old_route_targets).collect();
        let updated_routes: HashSet<&Endpoint> =
            new_route_targets.intersection(&old_route_targets).collect();
        for added_route in added_routes {
            debug!("New route available for {added_route}");
            self.deliver_remote_bundles(added_route, ctx);
        }
        for updated_route in updated_routes {
            debug!("Updated route available for {updated_route}");
            self.deliver_remote_bundles(updated_route, ctx);
        }
    }
}

impl Daemon {
    fn deliver_local_bundles(&mut self, destination: &Endpoint, ctx: &mut Context<Self>) {
        let Some(sender) = self.local_connections.get(destination) else {
            return;
        };

        if let Some(queue) = self.local_bundles.get_mut(destination) {
            while let Some(bundle) = queue.pop_front() {
                debug!(
                    "locally delivering bundle {:?}",
                    &bundle.get_primary_block()
                );
                assert!(
                    bundle.get_primary_block().fragment_offset.is_none(),
                    "Bundle is a fragment. It should have been reassembled before calling this"
                );

                match sender.try_send(ClientDeliverBundle {
                    bundle: bundle.clone(),
                    responder: ctx.address().recipient(),
                }) {
                    Ok(()) => self
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
                            warn!(
                                "Client for endpoint {destination} disconnected while sending bundles. Queueing..."
                            );
                            queue.push_back(bundle);
                            self.local_connections.remove(destination);
                            return;
                        }
                    },
                }
            }
        }
    }

    fn deliver_remote_bundles(&mut self, destination: &Endpoint, ctx: &mut Context<Self>) {
        let destination = destination.get_node_endpoint();
        let Some(route) = self.remote_routes.get(&destination) else {
            return;
        };
        let Some(sender) = self.remote_connections.get(&route.next_hop) else {
            return;
        };
        let Some(nexthopinfo) = self.remote_routes.get(&route.next_hop) else {
            return;
        };
        if route.next_hop != nexthopinfo.next_hop {
            warn!(
                "Route {destination} points to nexthop {} that is not directly connected",
                route.next_hop
            );
            return;
        }
        // This gets the smaller max_bundle_size for both of them, ignoring any Nones
        let max_bundle_size = match route.max_size {
            Some(ms) => Some(match nexthopinfo.max_size {
                Some(s_ms) => ms.min(s_ms),
                None => ms,
            }),
            None => nexthopinfo.max_size,
        };

        if let Some(queue) = self.remote_bundles.get_mut(&destination) {
            let mut visited: HashSet<String> = HashSet::new();
            while let Some(bundle) = queue.pop_front() {
                if visited.contains(&bundle.get_id()) {
                    queue.push_front(bundle);
                    break;
                }
                debug!(
                    "forwarding bundle {:?} to {:?}",
                    &bundle.get_primary_block(),
                    destination
                );

                match max_bundle_size {
                    Some(mbs) if bundle.get_bundle_size() > mbs => {
                        if bundle
                            .get_primary_block()
                            .bundle_processing_flags
                            .contains(BundleFlags::MUST_NOT_FRAGMENT)
                            || bundle.get_bundle_min_size().is_some()
                                && bundle.get_bundle_min_size().unwrap() > mbs
                        {
                            debug!(
                                "Bundle can not be fragmented as we can not get it that small. Queueing it"
                            );
                            visited.insert(bundle.get_id());
                            queue.push_back(bundle);
                        } else {
                            crate::bundlestorageagent::agent::Daemon::from_registry().do_send(
                                FragmentBundle {
                                    bundleref: bundle,
                                    target_size: mbs,
                                },
                            );
                        }
                        continue;
                    }
                    Some(_) | None => {}
                }

                match sender.try_send(AgentForwardBundle {
                    bundle: bundle.clone(),
                    responder: ctx.address().recipient(),
                }) {
                    Ok(()) => self
                        .bundles_pending_forwarding
                        .entry(destination.clone())
                        .or_default()
                        .push(bundle),
                    Err(e) => match e {
                        SendError::Full(afb) => {
                            debug!(
                                "Can not continue forwarding to {}. Waiting for some space in the queue",
                                route.next_hop
                            );
                            let AgentForwardBundle { bundle, .. } = afb;
                            queue.push_back(bundle);
                            return;
                        }
                        SendError::Closed(_) => {
                            warn!(
                                "Peer for endpoint {destination} disconnected while forwarding bundles. Queueing..."
                            );
                            queue.push_back(bundle);
                            self.remote_connections.remove(&destination);
                            return;
                        }
                    },
                }
            }
        }
    }

    fn send_status_report(
        &mut self,
        bundle: &StoredBundleRef,
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
        let pb = bundle.get_primary_block();
        let ar = AdministrativeRecord::BundleStatusReport(BundleStatusReport {
            status_information: BundleStatusInformation {
                received_bundle: received_info,
                forwarded_bundle: forwarded_info,
                delivered_bundle: delivered_info,
                deleted_bundle: deleted_info,
            },
            reason,
            bundle_source: pb.source_node.clone(),
            bundle_creation_timestamp: pb.creation_timestamp.clone(),
            fragment_offset: pb.fragment_offset,
            fragment_length: pb.total_data_length,
        });
        match TryInto::<Vec<u8>>::try_into(ar) {
            Ok(data) => {
                let bundle_data = Bundle {
                    primary_block: PrimaryBlock {
                        version: 7,
                        bundle_processing_flags: BundleFlags::ADMINISTRATIVE_RECORD,
                        crc: CRCType::NoCRC,
                        destination_endpoint: pb.report_to.clone(),
                        source_node: self.endpoint.as_ref().unwrap().clone(),
                        report_to: self.endpoint.as_ref().unwrap().clone(),
                        creation_timestamp: CreationTimestamp {
                            creation_time: DtnTime::now(),
                            sequence_number: 0, // uniqueness guaranteed in BSA
                        },
                        lifetime: pb.lifetime,
                        fragment_offset: None,
                        total_data_length: None,
                    },
                    blocks: vec![CanonicalBlock {
                        block: Block::Payload(PayloadBlock {
                            data: data.as_slice(),
                        }),
                        block_flags: BlockFlags::empty(),
                        block_number: 1,
                        crc: CRCType::NoCRC,
                    }],
                }
                .try_into()
                .unwrap();
                debug!("Dispatching administrative record bundle {pb:?}");
                crate::bundlestorageagent::agent::Daemon::from_registry()
                    .do_send(StoreNewBundle { bundle_data });
            }
            Err(e) => {
                warn!("Error serializing bundle status report: {e:?}");
            }
        }
    }

    fn send_status_report_delivered(&mut self, bundle: &StoredBundleRef) {
        if !bundle
            .get_primary_block()
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

    fn send_status_report_forwarded(&mut self, bundle: &StoredBundleRef) {
        if !bundle
            .get_primary_block()
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

    fn send_status_report_received(&mut self, bundle: &StoredBundleRef) {
        if !bundle
            .get_primary_block()
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

    fn send_status_report_deleted(&mut self, bundle: &StoredBundleRef, reason: BundleStatusReason) {
        if !bundle
            .get_primary_block()
            .bundle_processing_flags
            .contains(BundleFlags::BUNDLE_DELETION_STATUS_REQUESTED)
        {
            return;
        }
        self.send_status_report(bundle, reason, false, false, false, true);
    }

    // TODO: support Bundle Age
    fn forward_bundle(&self, sbr: &StoredBundleRef) -> Result<Vec<u8>, BundleStatusReason> {
        let data = sbr.get_bundle_data().unwrap();
        let mut bundle: Bundle = data
            .as_slice()
            .try_into()
            .expect("Validation already happened");
        if !self
            .endpoint
            .as_ref()
            .unwrap()
            .matches_node(&bundle.primary_block.source_node)
        {
            bundle.set_previous_node(self.endpoint.as_ref().unwrap());
        }
        if !bundle.inc_hop_count(HOP_LIMIT_DEFAULT) {
            return Err(BundleStatusReason::HopLimitExceeded);
        }
        Ok(bundle.try_into().expect("No way to fail"))
    }
}
