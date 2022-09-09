use log::{error, info};
use tokio::sync::{broadcast, mpsc};

mod bundleprotocolagent;
mod bundlestorageagent;
mod clientagent;
mod clientgrpcagent;
mod common;
mod converganceagent;
mod nodeagent;
mod routingagent;
mod tcpclconverganceagent;

use crate::common::{messages::Shutdown, settings::Settings};

use actix::{Actor, System};

#[actix_rt::main]
async fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    info!("Starting up");
    let settings: Settings = Settings::from_env();
    info!("Starting with settings: {:?}", settings);
    if let Some(tokio_tracing_port) = settings.tokio_tracing_port.clone() {
        info!("Initializing tokio tracing on port {}", tokio_tracing_port);
        console_subscriber::ConsoleLayer::builder()
            .server_addr(([127, 0, 0, 1], tokio_tracing_port.parse().unwrap()))
            .init();
    }

    let (notify_shutdown, _) = broadcast::channel::<()>(1);
    let (shutdown_complete_tx, mut shutdown_complete_rx) = mpsc::channel::<()>(1);

    let clientagent_addr = clientagent::agent::Daemon::default().start();

    let api_agent_task_shutdown_notifier = notify_shutdown.subscribe();
    let api_agent_task_shutdown_complete_tx_task = shutdown_complete_tx.clone();
    let api_agent_clientagent_addr = clientagent_addr.clone();
    let api_agent_task = tokio::task::Builder::new()
        .name("ApiAgent")
        .spawn(async move {
            match clientgrpcagent::agent::main(
                api_agent_task_shutdown_notifier,
                api_agent_task_shutdown_complete_tx_task,
                api_agent_clientagent_addr,
            )
            .await
            {
                Ok(_) => Ok(()),
                Err(e) => Err(e.to_string()),
            }
        });

    let tcpcl_server_addr = tcpclconverganceagent::agent::TCPCLServer::default().start();

    let tcpcl_listener_shutdown_notifier = notify_shutdown.subscribe();
    let tcpcl_listener_shutdown_complete_tx_task = shutdown_complete_tx.clone();
    let tcpcl_listener = tcpclconverganceagent::agent::tcpcl_listener(
        tcpcl_listener_shutdown_notifier,
        tcpcl_listener_shutdown_complete_tx_task,
        tcpcl_server_addr.clone(),
    )
    .await
    .unwrap();

    let ctrl_c = tokio::signal::ctrl_c();

    tokio::select! {
        res = api_agent_task => {
            if let Ok(Err(e)) = res {
                error!("something bad happened with the client grpc agent {:?}. Aborting...", e);
            }
        }
        res = tcpcl_listener => {
            if let Err(_) = res {
                error!("something bad happened with the tcpcl listener. Aborting...");
            }
        }
        _ = ctrl_c => {
            info!("Shutting down");
        }
    }

    info!("Stopping external connections");
    // Stolen from: https://github.com/tokio-rs/mini-redis/blob/master/src/server.rs
    // When `notify_shutdown` is dropped, all tasks which have `subscribe`d will
    // receive the shutdown signal and can exit
    drop(notify_shutdown);
    // Drop final `Sender` so the `Receiver` below can complete
    drop(shutdown_complete_tx);

    info!("Stopping individual actors");
    clientagent_addr.do_send(Shutdown {});
    tcpcl_server_addr.do_send(Shutdown {});

    info!("Now stopping actor system");
    System::current().stop();

    // Wait for all active connections to finish processing. As the `Sender`
    // handle held by the listener has been dropped above, the only remaining
    // `Sender` instances are held by connection handler tasks. When those drop,
    // the `mpsc` channel will close and `recv()` will return `None`.
    let _ = shutdown_complete_rx.recv().await;

    info!("All done, see you");
}
