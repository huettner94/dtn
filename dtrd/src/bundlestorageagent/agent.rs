use async_trait::async_trait;
use bp7::{bundle::Bundle, endpoint::Endpoint};
use log::{debug, error, warn};
use tokio::sync::{mpsc, oneshot};

use crate::common::settings::Settings;

use super::{messages::BSARequest, StoredBundle};

pub struct Daemon {
    bundles: Vec<StoredBundle>,
    channel_receiver: Option<mpsc::Receiver<BSARequest>>,
}

#[async_trait]
impl crate::common::agent::Daemon for Daemon {
    type MessageType = BSARequest;

    async fn new(_: &Settings) -> Self {
        Daemon {
            bundles: Vec::new(),
            channel_receiver: None,
        }
    }

    fn get_agent_name(&self) -> &'static str {
        "BSA"
    }

    fn get_channel_receiver(&mut self) -> Option<mpsc::Receiver<Self::MessageType>> {
        self.channel_receiver.take()
    }

    async fn on_shutdown(&mut self) {
        if self.bundles.len() != 0 {
            warn!(
                "BSA had {} bundles left over, they will be gone now.",
                self.bundles.len()
            );
        }
    }

    async fn handle_message(&mut self, msg: BSARequest) {
        match msg {
            BSARequest::StoreBundle { bundle, responder } => {
                self.message_store_bundle(bundle, responder).await;
            }
            BSARequest::DeleteBundle { bundle } => {
                self.message_delete_bundle(bundle).await;
            }
            BSARequest::GetBundleForDestination {
                destination,
                bundles,
            } => {
                self.message_get_bundle_for_destination(destination, bundles)
                    .await;
            }
            BSARequest::GetBundleForNode {
                destination,
                bundles,
            } => {
                self.message_get_bundle_for_node(destination, bundles).await;
            }
        }
    }
}

impl Daemon {
    pub fn init_channels(&mut self) -> mpsc::Sender<BSARequest> {
        let (channel_sender, channel_receiver) = mpsc::channel::<BSARequest>(1);
        self.channel_receiver = Some(channel_receiver);
        return channel_sender;
    }

    async fn message_store_bundle(
        &mut self,
        bundle: Bundle,
        responder: oneshot::Sender<Result<StoredBundle, ()>>,
    ) {
        debug!("Storing Bundle {:?} for later", bundle);
        let sb: StoredBundle = bundle.into();
        self.bundles.push(sb.clone());
        if let Err(_) = responder.send(Ok(sb)) {
            error!("Error sending stored bundle result");
        };
    }

    async fn message_delete_bundle(&mut self, bundle: StoredBundle) {
        match self.bundles.iter().position(|b| b == bundle) {
            Some(idx) => {
                self.bundles.remove(idx);
            }
            None => {}
        }
    }

    async fn message_get_bundle_for_destination(
        &mut self,
        destination: Endpoint,
        bundles: oneshot::Sender<Result<Vec<StoredBundle>, String>>,
    ) {
        let mut ret = Vec::new();
        let mut i = 0;
        while i < self.bundles.len() {
            if self.bundles[i]
                .get_bundle()
                .primary_block
                .destination_endpoint
                == destination
            {
                ret.push(self.bundles[i].clone());
            } else {
                i += 1;
            }
        }
        debug!(
            "Returning {} bundles for destination {}",
            ret.len(),
            destination
        );
        if let Err(e) = bundles.send(Ok(ret)) {
            error!("Error sending bundles to sender {:?}", e);
        }
    }

    async fn message_get_bundle_for_node(
        &mut self,
        destination: Endpoint,
        bundles: oneshot::Sender<Result<Vec<StoredBundle>, String>>,
    ) {
        let mut ret = Vec::new();
        let mut i = 0;
        while i < self.bundles.len() {
            if self.bundles[i]
                .get_bundle()
                .primary_block
                .destination_endpoint
                .matches_node(&destination)
            {
                ret.push(self.bundles[i].clone());
            } else {
                i += 1;
            }
        }
        debug!(
            "Returning {} bundles for destination {}",
            ret.len(),
            destination
        );
        if let Err(e) = bundles.send(Ok(ret)) {
            error!("Error sending bundles to sender {:?}", e);
        }
    }
}
