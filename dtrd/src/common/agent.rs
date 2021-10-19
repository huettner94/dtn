use async_trait::async_trait;
use log::info;
use tokio::sync::{broadcast, mpsc};

use crate::shutdown::Shutdown;

#[async_trait]
pub trait Daemon {
    type MessageType: Send;

    fn new() -> Self;

    fn get_agent_name(&self) -> &'static str;

    fn get_channel_receiver(&mut self) -> Option<mpsc::Receiver<Self::MessageType>>;

    fn validate(&self) {}

    async fn run(
        &mut self,
        shutdown_signal: broadcast::Receiver<()>,
        _sender: mpsc::Sender<()>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let channel_receiver = self.get_channel_receiver();
        if channel_receiver.is_none() {
            panic!("Must call init_cannel before calling run (also run may only be called once)");
        }
        self.validate();
        info!("{} starting...", self.get_agent_name());

        let mut shutdown = Shutdown::new(shutdown_signal);
        let mut receiver = channel_receiver.unwrap();

        while !shutdown.is_shutdown() {
            tokio::select! {
                res = receiver.recv() => {
                    if let Some(msg) = res {
                        self.handle_message(msg).await;
                    } else {
                        info!("{} can no longer receive messages. Exiting", self.get_agent_name());
                        return Ok(())
                    }
                }
                _ = shutdown.recv() => {
                    info!("{} received shutdown", self.get_agent_name());
                    receiver.close();
                    info!("{} will not allow more requests to be sent", self.get_agent_name());
                }
            }
        }

        while let Some(msg) = receiver.recv().await {
            self.handle_message(msg).await;
        }

        self.on_shutdown().await;

        info!("{} has shutdown. See you", self.get_agent_name());
        // _sender is explicitly dropped here
        Ok(())
    }

    async fn on_shutdown(&mut self) {}

    async fn handle_message(&mut self, msg: Self::MessageType);
}