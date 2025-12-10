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

use std::{path::PathBuf, sync::Arc};

use bp7::{bundle::Bundle, endpoint::Endpoint, time::DtnTime};
use log::{debug, info, warn};
use tokio::{fs, io::AsyncWriteExt};

use crate::{
    bundlestorageagent::{
        State, StoredBundleRef,
        messages::{EventBundleUpdated, UpdateBundle},
    },
    common::settings::Settings,
};

use super::{
    StoredBundle,
    messages::{
        EventNewBundleStored, FragmentBundle, GetBundleForDestination, GetBundleForNode,
        StoreBundle, StoreNewBundle,
    },
};
use actix::prelude::*;

#[derive(Default)]
pub struct Daemon {
    bundles: Vec<StoredBundle>,
    endpoint: Option<Endpoint>,
    storage_path: PathBuf,
    last_created_dtn_time: Option<DtnTime>,
    last_sequence_number: u64,
}

impl Actor for Daemon {
    type Context = Context<Self>;
    fn started(&mut self, ctx: &mut Context<Self>) {
        let settings = Settings::from_env();
        self.endpoint = Some(Endpoint::new(&settings.my_node_id).unwrap());
        self.storage_path = settings.bundle_storage_path.into();

        let storage_path = self.storage_path.clone();
        let fut = async move {
            info!("Loading existing bundles");
            let meta = fs::metadata(&storage_path).await;
            assert!(
                meta.is_ok(),
                "Bundle storage path must point to an existing directory"
            );
            if let Ok(m) = meta
                && !m.is_dir()
            {
                panic!("Bundle storage path must point to a valid directory");
            }

            let mut existing_bundles = Vec::new();

            let mut readdir = fs::read_dir(&storage_path)
                .await
                .expect("Failed to read existing bundles");

            while let Some(entry) = readdir
                .next_entry()
                .await
                .expect("Failed to read dir entry")
            {
                debug!(
                    "Loading existing bundle from {}",
                    entry.path().to_string_lossy()
                );
                let meta = entry.metadata().await.expect("Failed to read metadata");
                if !meta.is_file() {
                    warn!(
                        "Skip loading existing bundle {} as it is not a file",
                        entry.path().to_string_lossy()
                    );
                    continue;
                }

                let content = fs::read(entry.path()).await.expect("Failed to read bundle");
                let mut sb = StoredBundle::from(content);
                if sb.get_filename()
                    != entry
                        .path()
                        .file_name()
                        .expect("Can not happen")
                        .to_string_lossy()
                {
                    panic!("No idea how we ended up here, someone wrote something wrong");
                }
                info!("Loaded bundle {}", sb.get_id());
                sb.state = State::Valid;
                existing_bundles.push(sb);
            }

            existing_bundles
        };
        fut.into_actor(self)
            .then(|bundles, act, _ctx| {
                act.bundles = bundles;
                for bundle in &act.bundles {
                    crate::bundleprotocolagent::agent::Daemon::from_registry().do_send(
                        EventNewBundleStored {
                            bundle: bundle.get_ref(),
                        },
                    );
                }
                fut::ready(())
            })
            .wait(ctx);
    }

    fn stopped(&mut self, _ctx: &mut Context<Self>) {
        if !self.bundles.is_empty() {
            info!(
                "BSA had {} bundles left over, they will be resend on restart.",
                self.bundles.len()
            );
        }
    }
}

impl actix::Supervised for Daemon {}

impl SystemService for Daemon {}

impl Handler<StoreBundle> for Daemon {
    type Result = ();

    fn handle(&mut self, msg: StoreBundle, _ctx: &mut Context<Self>) -> Self::Result {
        let StoreBundle { bundle_data } = msg;
        self.store_bundle(bundle_data, None);
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

        assert!(
            bundle.primary_block.fragment_offset.is_none(),
            "Do not send fragments to StoreNewBundle"
        );

        let timestamp = bundle.primary_block.creation_timestamp.creation_time;
        let sequence_number = if Some(timestamp) == self.last_created_dtn_time {
            self.last_sequence_number += 1;
            self.last_sequence_number
        } else {
            self.last_created_dtn_time = Some(timestamp);
            self.last_sequence_number = 0;
            0
        };
        debug!("Decided sequence number {sequence_number:?} for new bundle",);
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
        let mut sb = if let Some(idx) = self.bundles.iter().position(|b| b == bundleref) {
            self.bundles.remove(idx)
        } else {
            warn!("Trying to fragment bundle, but could not find it. Not fragmenting it.");
            return;
        };

        match sb.get_bundle().fragment(target_size as usize) {
            Ok((bundles, first_min_size, min_size)) => {
                let mut iterator = bundles.into_iter();
                self.store_bundle(
                    iterator
                        .next()
                        .expect("There must always be at least one")
                        .try_into()
                        .unwrap(),
                    Some(first_min_size),
                );
                for bundle in iterator {
                    self.store_bundle(bundle.try_into().unwrap(), Some(min_size));
                }
            }
            Err(e) => match e {
                bp7::FragmentationError::SerializationError(e) => {
                    panic!("Error fragmenting bundle: {e:?}")
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

impl Handler<UpdateBundle> for Daemon {
    type Result = ();

    fn handle(&mut self, msg: UpdateBundle, ctx: &mut Context<Self>) {
        let UpdateBundle {
            bundleref,
            new_state,
            new_data,
        } = msg;
        if let Some(idx) = self.bundles.iter().position(|b| b == bundleref) {
            let mut bundle = self.bundles.remove(idx);

            if matches!(new_state, State::Valid) && !matches!(bundle.state, State::Valid) {
                // Since the bundle is now valid for the first time we should store it.
                // We need to exclude existing valid bundles, otherwise we redo this on startup.
                let mut path = self.storage_path.clone();
                path.push(bundle.get_filename());
                debug!("Storing bundle to {}", path.to_string_lossy());
                let data = bundle.bundle_data.clone();
                let fut = async move {
                    let mut file = fs::File::create_new(path).await?;
                    file.write_all(&data).await?;
                    file.sync_all().await?;
                    Ok(())
                };
                fut.into_actor(self)
                    .then(|res: std::io::Result<()>, _act, _ctx| {
                        if let Err(e) = res {
                            warn!("Failed to write: {e}");
                        }
                        fut::ready(())
                    })
                    .wait(ctx);
            }

            if matches!(
                new_state,
                State::Valid | State::DeliveryQueued | State::ForwardingQueued
            ) {
                bundle.state = new_state;
                if let Some(data) = new_data {
                    bundle.bundle_data = Arc::new(data);
                    // TODO: we need to write the file again
                }
                let sbr = bundle.get_ref();
                self.bundles.push(bundle);
                crate::bundleprotocolagent::agent::Daemon::from_registry()
                    .do_send(EventBundleUpdated { bundle: sbr });
            } else if matches!(
                new_state,
                State::Delivered | State::Forwarded | State::Invalid
            ) {
                // We are done, delete the file
                let mut path = self.storage_path.clone();
                path.push(bundle.get_filename());
                let fut = async move { fs::remove_file(path).await };
                fut.into_actor(self)
                    .then(|res, _act, _ctx| {
                        if let Err(e) = res {
                            warn!("Failed to delete bundle: {e}");
                        }
                        fut::ready(())
                    })
                    .spawn(ctx);
            }
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
    fn store_bundle(&mut self, bundle_data: Vec<u8>, min_size: Option<u64>) {
        let mut sb: StoredBundle = bundle_data.into();
        sb.min_size = min_size;
        let bundle: Bundle = sb.get_bundle();

        debug!("Storing Bundle {:?} for later", bundle.primary_block);
        let local = bundle
            .primary_block
            .destination_endpoint
            .matches_node(self.endpoint.as_ref().unwrap());

        if local {
            if bundle.primary_block.fragment_offset.is_some() {
                if let Some(defragmented) = self.try_defragment_bundle(&sb) {
                    let sbr = defragmented.get_ref();
                    self.bundles.push(defragmented);
                    crate::bundleprotocolagent::agent::Daemon::from_registry()
                        .do_send(EventNewBundleStored { bundle: sbr });
                }
            } else {
                let sbr = sb.get_ref();
                self.bundles.push(sb);
                crate::bundleprotocolagent::agent::Daemon::from_registry()
                    .do_send(EventNewBundleStored { bundle: sbr });
            }
        } else {
            let sbr = sb.get_ref();
            self.bundles.push(sb);
            crate::bundleprotocolagent::agent::Daemon::from_registry()
                .do_send(EventNewBundleStored { bundle: sbr });
        }
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
        if let Ok(bundledata) = Bundle::reassemble_bundles(fragments_ref) {
            let sb: StoredBundle = bundledata.into();
            debug!("Bundle {} sucessfully reassembled", sb.get_id());
            Some(sb)
        } else {
            self.bundles.append(&mut fragments);
            None
        }
    }
}
