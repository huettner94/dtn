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

use bp7::{bundle::Bundle, endpoint::Endpoint, time::DtnTime};
use log::{debug, warn};

use crate::{bundlestorageagent::StoredBundleRef, common::settings::Settings};

use super::{StoredBundle, messages::*};
use actix::prelude::*;

#[derive(Default)]
pub struct Daemon {
    bundles: Vec<StoredBundle>,
    endpoint: Option<Endpoint>,
    last_created_dtn_time: Option<DtnTime>,
    last_sequence_number: u64,
}

impl Actor for Daemon {
    type Context = Context<Self>;
    fn started(&mut self, _ctx: &mut Context<Self>) {
        let settings = Settings::from_env();
        self.endpoint = Some(Endpoint::new(&settings.my_node_id).unwrap());
    }

    fn stopped(&mut self, _ctx: &mut Context<Self>) {
        if !self.bundles.is_empty() {
            warn!(
                "BSA had {} bundles left over, they will be gone now.",
                self.bundles.len()
            );
        }
    }
}

impl actix::Supervised for Daemon {}

impl SystemService for Daemon {}

impl Handler<StoreBundle> for Daemon {
    type Result = Result<(), ()>;

    fn handle(&mut self, msg: StoreBundle, _ctx: &mut Context<Self>) -> Self::Result {
        let StoreBundle { bundle_data } = msg;
        let bundle: Bundle = bundle_data.as_slice().try_into().unwrap();

        if bundle
            .primary_block
            .source_node
            .matches_node(self.endpoint.as_ref().unwrap())
        {
            panic!(
                "Received a StoreBundle message but with us as the source node. Use StoreNewBundle instead!"
            )
        }

        self.store_bundle(bundle_data, None)
    }
}

impl Handler<StoreNewBundle> for Daemon {
    type Result = Result<(), ()>;

    fn handle(&mut self, msg: StoreNewBundle, _ctx: &mut Self::Context) -> Self::Result {
        let StoreNewBundle { bundle_data } = msg;
        let mut bundle: Bundle = bundle_data.as_slice().try_into().unwrap();

        if !bundle
            .primary_block
            .source_node
            .matches_node(self.endpoint.as_ref().unwrap())
        {
            panic!(
                "Received a StoreNewBundle message but with some other node as source node. Use StoreBundle instead!"
            )
        }

        if bundle.primary_block.fragment_offset.is_some() {
            panic!("Do not send fragments to StoreNewBundle")
        }

        let timestamp = bundle.primary_block.creation_timestamp.creation_time;
        let sequence_number = if Some(timestamp) == self.last_created_dtn_time {
            self.last_sequence_number += 1;
            self.last_sequence_number
        } else {
            self.last_created_dtn_time = Some(timestamp);
            self.last_sequence_number = 0;
            0
        };
        debug!(
            "Decided sequence number {:?} for new bundle",
            sequence_number
        );
        bundle.primary_block.creation_timestamp.sequence_number = sequence_number;

        debug!("Storing Bundle {:?} for later", bundle.primary_block);
        let bundle_data: Vec<u8> = bundle.try_into().unwrap();
        let sb: StoredBundle = bundle_data.into();
        let sb_ref = sb.get_ref();

        self.bundles.push(sb);
        crate::bundleprotocolagent::agent::Daemon::from_registry()
            .do_send(EventNewBundleStored { bundle: sb_ref });

        Ok(())
    }
}

impl Handler<FragmentBundle> for Daemon {
    type Result = ();

    fn handle(&mut self, msg: FragmentBundle, _ctx: &mut Self::Context) -> Self::Result {
        let FragmentBundle {
            bundleref,
            target_size,
        } = msg;
        let mut sb = match self.bundles.iter().position(|b| b == bundleref) {
            Some(idx) => self.bundles.remove(idx),
            None => {
                warn!("Trying to fragment bundle, but could not find it. Not fragmenting it.");
                return;
            }
        };

        match sb.get_bundle().fragment(target_size) {
            Ok((bundles, first_min_size, min_size)) => {
                let mut iterator = bundles.into_iter();
                self.store_bundle(
                    iterator
                        .next()
                        .expect("There must always be at least one")
                        .try_into()
                        .unwrap(),
                    Some(first_min_size),
                )
                .unwrap();
                for bundle in iterator {
                    self.store_bundle(bundle.try_into().unwrap(), Some(min_size))
                        .unwrap();
                }
            }
            Err(e) => match e {
                bp7::FragmentationError::SerializationError(e) => {
                    panic!("Error fragmenting bundle: {:?}", e)
                }
                bp7::FragmentationError::CanNotFragmentThatSmall(min_size) => {
                    sb.min_size = Some(min_size);
                    let sbr = sb.get_ref();
                    self.bundles.push(sb);
                    crate::bundleprotocolagent::agent::Daemon::from_registry()
                        .do_send(EventNewBundleStored { bundle: sbr });
                }
                bp7::FragmentationError::MustNotFragment => {
                    panic!("Attempted to fragment a bundle that must not be fragmented")
                }
                bp7::FragmentationError::BundleInvalid => {
                    panic!("Attempted to fragment a invalid bundle")
                }
            },
        }
    }
}

impl Handler<DeleteBundle> for Daemon {
    type Result = ();

    fn handle(&mut self, msg: DeleteBundle, _ctx: &mut Context<Self>) {
        let DeleteBundle { bundle } = msg;
        if let Some(idx) = self.bundles.iter().position(|b| b == bundle) {
            self.bundles.remove(idx);
        }
    }
}

impl Handler<GetBundleForDestination> for Daemon {
    type Result = Result<Vec<StoredBundleRef>, String>;

    fn handle(&mut self, msg: GetBundleForDestination, _ctx: &mut Context<Self>) -> Self::Result {
        let GetBundleForDestination { destination } = msg;
        let mut ret = Vec::new();
        for i in 0..self.bundles.len() {
            if self.bundles[i]
                .get_bundle()
                .primary_block
                .destination_endpoint
                == destination
            {
                ret.push(self.bundles[i].get_ref());
            }
        }
        debug!(
            "Returning {} bundles for destination {}",
            ret.len(),
            destination
        );
        Ok(ret)
    }
}

impl Handler<GetBundleForNode> for Daemon {
    type Result = Result<Vec<StoredBundleRef>, String>;

    fn handle(&mut self, msg: GetBundleForNode, _ctx: &mut Context<Self>) -> Self::Result {
        let GetBundleForNode { destination } = msg;
        let mut ret = Vec::new();
        for i in 0..self.bundles.len() {
            if self.bundles[i]
                .get_bundle()
                .primary_block
                .destination_endpoint
                .matches_node(&destination)
            {
                ret.push(self.bundles[i].get_ref());
            }
        }
        debug!(
            "Returning {} bundles for destination {}",
            ret.len(),
            destination
        );
        Ok(ret)
    }
}

impl Daemon {
    fn store_bundle(&mut self, bundle_data: Vec<u8>, min_size: Option<u64>) -> Result<(), ()> {
        let mut sb: StoredBundle = bundle_data.into();
        sb.min_size = min_size;
        let bundle: Bundle = sb.get_bundle();

        debug!("Storing Bundle {:?} for later", bundle.primary_block);
        let local = bundle
            .primary_block
            .destination_endpoint
            .matches_node(self.endpoint.as_ref().unwrap());

        if local {
            match bundle.primary_block.fragment_offset.is_some() {
                true => {
                    if let Some(defragmented) = self.try_defragment_bundle(&sb) {
                        let sbr = defragmented.get_ref();
                        self.bundles.push(defragmented);
                        crate::bundleprotocolagent::agent::Daemon::from_registry()
                            .do_send(EventNewBundleStored { bundle: sbr });
                    }
                }
                false => {
                    let sbr = sb.get_ref();
                    self.bundles.push(sb);
                    crate::bundleprotocolagent::agent::Daemon::from_registry()
                        .do_send(EventNewBundleStored { bundle: sbr });
                }
            }
        } else {
            let sbr = sb.get_ref();
            self.bundles.push(sb);
            crate::bundleprotocolagent::agent::Daemon::from_registry()
                .do_send(EventNewBundleStored { bundle: sbr });
        }

        Ok(())
    }

    fn try_defragment_bundle(&mut self, bundle: &StoredBundle) -> Option<StoredBundle> {
        let requested_primary_block = bundle.get_bundle().primary_block.clone();

        let mut i = 0;
        let mut fragments: Vec<StoredBundle> = Vec::new();
        while i < self.bundles.len() {
            if self.bundles[i]
                .get_bundle()
                .primary_block
                .equals_ignoring_fragment_info(&requested_primary_block)
            {
                fragments.push(self.bundles.remove(i));
            } else {
                i += 1;
            }
        }

        let fragments_ref = fragments.iter().map(|b| b.get_bundle()).collect();
        match Bundle::reassemble_bundles(fragments_ref) {
            Ok(bundledata) => {
                let sb: StoredBundle = bundledata.into();
                debug!("Bundle {} sucessfully reassembled", sb.get_id());
                Some(sb)
            }
            Err(_) => {
                self.bundles.append(&mut fragments);
                None
            }
        }
    }
}
