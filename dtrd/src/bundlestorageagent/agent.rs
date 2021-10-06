use async_trait::async_trait;
use bp7::bundle::Bundle;
use log::{debug, warn};
use tokio::sync::mpsc;

use super::messages::BSARequest;

pub struct Daemon {
    bundles: Vec<Bundle>,
    channel_receiver: Option<mpsc::Receiver<BSARequest>>,
}

#[async_trait]
impl crate::common::agent::Daemon for Daemon {
    type MessageType = BSARequest;

    fn new() -> Self {
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
            BSARequest::StoreBundle { bundle } => {
                debug!("Storing Bundle {:?} for later", bundle);
                self.bundles.push(bundle);
            }
            BSARequest::GetBundleForDestination {
                destination,
                bundles,
            } => {
                let mut ret = Vec::new();
                let mut i = 0;
                while i < self.bundles.len() {
                    if self.bundles[i].primary_block.destination_endpoint == destination {
                        ret.push(self.bundles.remove(i));
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
                    warn!("Error sending bundles to sender {:?}", e);
                }
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
}
