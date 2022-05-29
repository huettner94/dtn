use async_trait::async_trait;
use bp7::{bundle::Bundle, endpoint::Endpoint};
use log::{debug, error, warn};
use tokio::sync::{mpsc, oneshot};

use crate::common::settings::Settings;

use super::{messages::BSARequest, StoredBundle};

pub struct Daemon {
    bundles: Vec<StoredBundle>,
    channel_receiver: Option<mpsc::UnboundedReceiver<BSARequest>>,
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

    fn get_channel_receiver(&mut self) -> Option<mpsc::UnboundedReceiver<Self::MessageType>> {
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
            BSARequest::TryDefragmentBundle { bundle, responder } => {
                self.message_try_defragment_bundle(bundle, responder).await;
            }
        }
    }
}

impl Daemon {
    pub fn init_channels(&mut self) -> mpsc::UnboundedSender<BSARequest> {
        let (channel_sender, channel_receiver) = mpsc::unbounded_channel::<BSARequest>();
        self.channel_receiver = Some(channel_receiver);
        return channel_sender;
    }

    async fn message_store_bundle(
        &mut self,
        bundle: Bundle,
        responder: oneshot::Sender<Result<StoredBundle, ()>>,
    ) {
        debug!("Storing Bundle {:?} for later", bundle.primary_block);
        let res: Result<StoredBundle, ()> = match TryInto::<StoredBundle>::try_into(bundle) {
            Ok(sb) => {
                self.bundles.push(sb.clone());
                Ok(sb)
            }
            Err(e) => {
                error!("Error converting bundle to StoredBundle: {:?}", e);
                Err(())
            }
        };

        if let Err(_) = responder.send(res) {
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
        if let Err(e) = bundles.send(Ok(ret)) {
            error!("Error sending bundles to sender {:?}", e);
        }
    }

    async fn message_try_defragment_bundle(
        &mut self,
        bundle: StoredBundle,
        responder: oneshot::Sender<Result<Option<StoredBundle>, ()>>,
    ) {
        let requested_primary_block = &bundle.get_bundle().primary_block;
        let mut i = 0;
        let mut fragments: Vec<Bundle> = Vec::new();
        while i < self.bundles.len() {
            if self.bundles[i]
                .bundle
                .primary_block
                .equals_ignoring_fragment_info(requested_primary_block)
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
            if let Err(e) = responder.send(Ok(Some(reassembled.drain(0..1).next().unwrap()))) {
                error!("Error sending reassembled bundle response: {:?}", e);
            }
        } else {
            self.bundles.append(&mut reassembled);
            if let Err(e) = responder.send(Ok(None)) {
                error!("Error sending empty bundle response: {:?}", e);
            }
        }
    }
}
