use bp7::{bundle::Bundle, endpoint::Endpoint, time::DtnTime};
use log::{debug, error, warn};

use crate::common::settings::Settings;

use super::{messages::*, StoredBundle};
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
        if self.bundles.len() != 0 {
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
        let StoreBundle { bundle } = msg;

        if bundle
            .primary_block
            .source_node
            .matches_node(&self.endpoint.as_ref().unwrap())
        {
            panic!("Received a StoreBundle message but with us as the source node. Use StoreNewBundle instead!")
        }

        self.store_bundle(bundle, None)
    }
}

impl Handler<StoreNewBundle> for Daemon {
    type Result = Result<(), ()>;

    fn handle(&mut self, msg: StoreNewBundle, _ctx: &mut Self::Context) -> Self::Result {
        let StoreNewBundle { mut bundle } = msg;

        if !bundle
            .primary_block
            .source_node
            .matches_node(&self.endpoint.as_ref().unwrap())
        {
            panic!("Received a StoreNewBundle message but with some other node as source node. Use StoreBundle instead!")
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
        let res: Result<(), ()> = match TryInto::<StoredBundle>::try_into(bundle) {
            Ok(sb) => {
                self.bundles.push(sb.clone());
                crate::bundleprotocolagent::agent::Daemon::from_registry()
                    .do_send(EventNewBundleStored { bundle: sb });

                Ok(())
            }
            Err(e) => {
                error!("Error converting bundle to StoredBundle: {:?}", e);
                Err(())
            }
        };
        res
    }
}

impl Handler<FragmentBundle> for Daemon {
    type Result = ();

    fn handle(&mut self, msg: FragmentBundle, _ctx: &mut Self::Context) -> Self::Result {
        let FragmentBundle {
            bundle,
            target_size,
        } = msg;
        match self.bundles.iter().position(|b| b == bundle) {
            Some(idx) => {
                self.bundles.remove(idx);
            }
            None => {
                warn!("Trying to fragment bundle, but could not find it. Not fragmenting it.");
                return;
            }
        }

        // TODO: this is more cloning than probably necessary
        match bundle.get_bundle().clone().fragment(target_size) {
            Ok((bundles, first_min_size, min_size)) => {
                let mut iterator = bundles.into_iter();
                self.store_bundle(iterator.next().unwrap(), Some(first_min_size))
                    .unwrap();
                for bundle in iterator {
                    self.store_bundle(bundle, Some(min_size)).unwrap();
                }
            }
            Err(e) => match e {
                bp7::FragmentationError::SerializationError(e) => {
                    panic!("Error fragmenting bundle: {:?}", e)
                }
                bp7::FragmentationError::CanNotFragmentThatSmall(min_size) => {
                    let real_bundle = bundle.get_bundle().clone();
                    self.store_bundle(real_bundle, Some(min_size)).unwrap();
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
        match self.bundles.iter().position(|b| b == bundle) {
            Some(idx) => {
                self.bundles.remove(idx);
            }
            None => {}
        }
    }
}

impl Handler<GetBundleForDestination> for Daemon {
    type Result = Result<Vec<StoredBundle>, String>;

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
                ret.push(self.bundles[i].clone());
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
    type Result = Result<Vec<StoredBundle>, String>;

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
                ret.push(self.bundles[i].clone());
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
    fn store_bundle(&mut self, bundle: Bundle, min_size: Option<u64>) -> Result<(), ()> {
        debug!("Storing Bundle {:?} for later", bundle.primary_block);
        let local = bundle
            .primary_block
            .destination_endpoint
            .matches_node(&self.endpoint.as_ref().unwrap());
        let res: Result<(), ()> = match TryInto::<StoredBundle>::try_into(bundle) {
            Ok(mut sb) => {
                sb.min_size = min_size;
                self.bundles.push(sb.clone());

                if local {
                    match sb.get_bundle().primary_block.fragment_offset.is_some() {
                        true => match self.try_defragment_bundle(&sb) {
                            Some(defragmented) => {
                                crate::bundleprotocolagent::agent::Daemon::from_registry().do_send(
                                    EventNewBundleStored {
                                        bundle: defragmented,
                                    },
                                );
                            }
                            None => {}
                        },
                        false => {
                            crate::bundleprotocolagent::agent::Daemon::from_registry()
                                .do_send(EventNewBundleStored { bundle: sb });
                        }
                    }
                } else {
                    crate::bundleprotocolagent::agent::Daemon::from_registry()
                        .do_send(EventNewBundleStored { bundle: sb });
                }

                Ok(())
            }
            Err(e) => {
                error!("Error converting bundle to StoredBundle: {:?}", e);
                Err(())
            }
        };
        res
    }

    fn try_defragment_bundle(&mut self, bundle: &StoredBundle) -> Option<StoredBundle> {
        let requested_primary_block = bundle.get_bundle().primary_block.clone();
        let mut i = 0;
        let mut fragments: Vec<Bundle> = Vec::new();
        while i < self.bundles.len() {
            if self.bundles[i]
                .bundle
                .primary_block
                .equals_ignoring_fragment_info(&requested_primary_block)
            {
                fragments.push(self.bundles.remove(i).bundle.as_ref().clone()); // TODO: This is a full clone and probably bad
            } else {
                i += 1;
            }
        }
        let mut reassembled: Vec<StoredBundle> = Bundle::reassemble_bundles(fragments)
            .into_iter()
            .map(|frag| frag.try_into().expect("This can not happen"))
            .collect();
        if reassembled.len() == 1
            && reassembled[0]
                .get_bundle()
                .primary_block
                .fragment_offset
                .is_none()
        {
            let reassembled_bundle = reassembled.drain(0..1).next().unwrap();
            self.bundles.push(reassembled_bundle.clone());
            Some(reassembled_bundle)
        } else {
            self.bundles.append(&mut reassembled);
            None
        }
    }
}
