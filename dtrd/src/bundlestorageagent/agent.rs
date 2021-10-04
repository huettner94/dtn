use bp7::bundle::Bundle;
use log::{debug, info, warn};
use tokio::sync::{broadcast, mpsc};

use crate::shutdown::Shutdown;

use super::messages::BSARequest;

pub struct Daemon {
    bundles: Vec<Bundle>,
    channel_receiver: Option<mpsc::Receiver<BSARequest>>,
}

impl Daemon {
    pub fn new() -> Self {
        Daemon {
            bundles: Vec::new(),
            channel_receiver: None,
        }
    }

    pub fn init_channels(&mut self) -> mpsc::Sender<BSARequest> {
        let (channel_sender, channel_receiver) = mpsc::channel::<BSARequest>(1);
        self.channel_receiver = Some(channel_receiver);
        return channel_sender;
    }

    pub async fn run(
        mut self,
        shutdown_signal: broadcast::Receiver<()>,
        _sender: mpsc::Sender<()>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        if self.channel_receiver.is_none() {
            panic!("Must call init_cannel before calling run (also run may only be called once)");
        }
        info!("BSA starting...");

        let mut shutdown = Shutdown::new(shutdown_signal);
        let mut receiver = self.channel_receiver.take().unwrap();

        while !shutdown.is_shutdown() {
            tokio::select! {
                res = receiver.recv() => {
                    if let Some(msg) = res {
                        self.handle_message(msg).await;
                    } else {
                        info!("BSA can no longer receive messages. Exiting");
                        return Ok(())
                    }
                }
                _ = shutdown.recv() => {
                    info!("BSA received shutdown");
                    receiver.close();
                    info!("BSA will not allow more requests to be sent");
                }
            }
        }

        while let Some(msg) = receiver.recv().await {
            self.handle_message(msg).await;
        }

        if self.bundles.len() != 0 {
            warn!(
                "BSA had {} bundles left over, they will be gone now.",
                self.bundles.len()
            );
        }

        info!("BSA has shutdown. See you");
        // _sender is explicitly dropped here
        Ok(())
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
