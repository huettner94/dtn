use futures_util::Future;
use log::info;
use tokio::sync::{broadcast, mpsc};

mod bundleprotocolagent;
mod bundlestorageagent;
mod clientagent;
mod clientgrpcagent;
mod common;
mod shutdown;

use crate::common::agent::Daemon;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    info!("Starting up");
    runserver(tokio::signal::ctrl_c()).await?;
    Ok(())
}

async fn runserver(ctrl_c: impl Future) -> Result<(), Box<dyn std::error::Error>> {
    let (notify_shutdown, _) = broadcast::channel::<()>(1);
    let (shutdown_complete_tx, mut shutdown_complete_rx) = mpsc::channel::<()>(1);

    let mut bundle_storage_agent = bundlestorageagent::agent::Daemon::new();
    let bsa_sender = bundle_storage_agent.init_channels();

    let mut bundle_protocol_agent = bundleprotocolagent::agent::Daemon::new();
    let bpa_sender = bundle_protocol_agent.init_channels(bsa_sender.clone());

    let mut client_agent = clientagent::agent::Daemon::new();
    let client_agent_sender = client_agent.init_channels(bpa_sender.clone());

    bundle_protocol_agent.set_client_agent(client_agent_sender.clone());

    let bpa_task_shutdown_notifier = notify_shutdown.subscribe();
    let bpa_task_shutdown_complete_tx_task = shutdown_complete_tx.clone();
    let bpa_task = tokio::spawn(async move {
        match bundle_protocol_agent
            .run(
                bpa_task_shutdown_notifier,
                bpa_task_shutdown_complete_tx_task,
            )
            .await
        {
            Ok(_) => Ok(()),
            Err(e) => Err(e.to_string()),
        }
    });

    let bsa_task_shutdown_notifier = notify_shutdown.subscribe();
    let bsa_task_shutdown_complete_tx_task = shutdown_complete_tx.clone();
    let bsa_task = tokio::spawn(async move {
        match bundle_storage_agent
            .run(
                bsa_task_shutdown_notifier,
                bsa_task_shutdown_complete_tx_task,
            )
            .await
        {
            Ok(_) => Ok(()),
            Err(e) => Err(e.to_string()),
        }
    });

    let client_agent_task_shutdown_notifier = notify_shutdown.subscribe();
    let client_agent_task_shutdown_complete_tx_task = shutdown_complete_tx.clone();
    let client_agent_task = tokio::spawn(async move {
        match client_agent
            .run(
                client_agent_task_shutdown_notifier,
                client_agent_task_shutdown_complete_tx_task,
            )
            .await
        {
            Ok(_) => Ok(()),
            Err(e) => Err(e.to_string()),
        }
    });

    let api_agent_task_shutdown_notifier = notify_shutdown.subscribe();
    let api_agent_task_shutdown_complete_tx_task = shutdown_complete_tx.clone();
    let api_agent_task_client_agent_sender = client_agent_sender.clone();
    let api_agent_task = tokio::spawn(async move {
        match clientgrpcagent::agent::main(
            api_agent_task_shutdown_notifier,
            api_agent_task_shutdown_complete_tx_task,
            api_agent_task_client_agent_sender,
        )
        .await
        {
            Ok(_) => Ok(()),
            Err(e) => Err(e.to_string()),
        }
    });

    tokio::select! {
        res = api_agent_task => {
            if let Ok(Err(e)) = res {
                info!("something bad happened with the apiagent {:?}. Aborting...", e);
            }
        }
        res = bpa_task => {
            if let Ok(Err(e)) = res {
                info!("something bad happened with the bpa agent {:?}. Aborting...", e);
            }
        }
        res = bsa_task => {
            if let Ok(Err(e)) = res {
                info!("something bad happened with the bsa agent {:?}. Aborting...", e);
            }
        }
        res = client_agent_task => {
            if let Ok(Err(e)) = res {
                info!("something bad happened with the client agent {:?}. Aborting...", e);
            }
        }
        _ = ctrl_c => {
            info!("Shutting down");
        }
    }

    // Stolen from: https://github.com/tokio-rs/mini-redis/blob/master/src/server.rs
    // When `notify_shutdown` is dropped, all tasks which have `subscribe`d will
    // receive the shutdown signal and can exit
    drop(notify_shutdown);
    // Drop final `Sender` so the `Receiver` below can complete
    drop(shutdown_complete_tx);
    drop(bpa_sender);

    // Wait for all active connections to finish processing. As the `Sender`
    // handle held by the listener has been dropped above, the only remaining
    // `Sender` instances are held by connection handler tasks. When those drop,
    // the `mpsc` channel will close and `recv()` will return `None`.
    let _ = shutdown_complete_rx.recv().await;

    info!("Shutdown complete. Goodbye :)");

    Ok(())
}
