use futures_util::Future;
use log::info;
use tokio::sync::{broadcast, mpsc};

mod apiagent;
mod bundleprotocolagent;

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

    let shutdown_notifier = notify_shutdown.subscribe();
    let shutdown_complete_tx_task = shutdown_complete_tx.clone();

    let api_agent_task = tokio::spawn(async move {
        match apiagent::main(shutdown_notifier, shutdown_complete_tx_task).await {
            Ok(_) => Ok(()),
            Err(e) => Err(e.to_string()),
        }
    });

    tokio::select! {
        res = api_agent_task => {
            if let Ok(Err(e)) = res {
                info!("something happened with the apiagent {:?}", e);
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

    // Wait for all active connections to finish processing. As the `Sender`
    // handle held by the listener has been dropped above, the only remaining
    // `Sender` instances are held by connection handler tasks. When those drop,
    // the `mpsc` channel will close and `recv()` will return `None`.
    let _ = shutdown_complete_rx.recv().await;

    info!("Shutdown complete. Goodbye :)");

    Ok(())
}
