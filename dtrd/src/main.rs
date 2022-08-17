use futures_util::Future;
use log::info;
use tokio::{
    sync::{broadcast, mpsc},
    task::JoinHandle,
};

mod bundleprotocolagent;
mod bundlestorageagent;
mod clientagent;
//mod clientgrpcagent;
mod common;
mod converganceagent;
mod nodeagent;
mod routingagent;
//mod tcpclconverganceagent;

use crate::common::{agent::Daemon, settings::Settings};

fn spawn_task(
    name: &str,
    mut daemon: impl Daemon + Send + 'static,
    notify_shutdown: &broadcast::Sender<()>,
    shutdown_complete: &mpsc::Sender<()>,
) -> JoinHandle<Result<(), String>> {
    let shutdown_notifier = notify_shutdown.subscribe();
    let shutdown_complete_tx_task = shutdown_complete.clone();
    tokio::task::Builder::new().name(name).spawn(async move {
        match daemon
            .run(shutdown_notifier, shutdown_complete_tx_task)
            .await
        {
            Ok(_) => Ok(()),
            Err(e) => Err(e.to_string()),
        }
    })
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    info!("Starting up");
    let settings: Settings = Settings::from_env();
    info!("Starting with settings: {:?}", settings);
    if let Some(tokio_tracing_port) = settings.tokio_tracing_port.clone() {
        info!("Initializing tokio tracing on port {}", tokio_tracing_port);
        console_subscriber::ConsoleLayer::builder()
            .server_addr(([127, 0, 0, 1], tokio_tracing_port.parse()?))
            .init();
    }
    runserver(tokio::signal::ctrl_c(), settings).await?;
    Ok(())
}

async fn runserver(
    ctrl_c: impl Future,
    settings: Settings,
) -> Result<(), Box<dyn std::error::Error>> {
    let (notify_shutdown, _) = broadcast::channel::<()>(1);
    let (shutdown_complete_tx, mut shutdown_complete_rx) = mpsc::channel::<()>(1);

    let mut bundle_storage_agent = bundlestorageagent::agent::Daemon::new(&settings).await;
    let bsa_sender = bundle_storage_agent.init_channels();

    let mut routing_agent = routingagent::agent::Daemon::new(&settings).await;
    let routing_agent_sender = routing_agent.init_channels();

    let mut bundle_protocol_agent = bundleprotocolagent::agent::Daemon::new(&settings).await;
    let bpa_sender =
        bundle_protocol_agent.init_channels(bsa_sender.clone(), routing_agent_sender.clone());

    let mut convergance_agent = converganceagent::agent::Daemon::new(&settings).await;
    let convergance_agent_sender = convergance_agent.init_channels(bpa_sender.clone());

    let mut tcpcl_agent = tcpclconverganceagent::agent::Daemon::new(&settings).await;
    let tcpcl_agent_sender = tcpcl_agent.init_channels(convergance_agent_sender.clone());

    let mut node_agent = nodeagent::agent::Daemon::new(&settings).await;
    let node_agent_sender = node_agent.init_channels(
        convergance_agent_sender.clone(),
        routing_agent_sender.clone(),
    );

    convergance_agent.set_senders(node_agent_sender.clone(), tcpcl_agent_sender.clone());
    routing_agent.set_senders(bpa_sender.clone());

    let mut client_agent = clientagent::agent::Daemon::new(&settings).await;
    let client_agent_sender = client_agent.init_channels(
        bpa_sender.clone(),
        node_agent_sender.clone(),
        routing_agent_sender.clone(),
    );

    bundle_protocol_agent.set_agents(
        client_agent_sender.clone(),
        convergance_agent_sender.clone(),
    );

    let bpa_task = spawn_task(
        "BundleProtocolAgent",
        bundle_protocol_agent,
        &notify_shutdown,
        &shutdown_complete_tx,
    );

    let routing_agent_task = spawn_task(
        "RoutingAgent",
        routing_agent,
        &notify_shutdown,
        &shutdown_complete_tx,
    );

    let bsa_task = spawn_task(
        "BundleStorageAgent",
        bundle_storage_agent,
        &notify_shutdown,
        &shutdown_complete_tx,
    );

    let client_agent_task = spawn_task(
        "ClientAgent",
        client_agent,
        &notify_shutdown,
        &shutdown_complete_tx,
    );

    let api_agent_task_shutdown_notifier = notify_shutdown.subscribe();
    let api_agent_task_shutdown_complete_tx_task = shutdown_complete_tx.clone();
    let api_agent_task_client_agent_sender = client_agent_sender.clone();
    let api_agent_task_settings = settings.clone();
    let api_agent_task = tokio::task::Builder::new()
        .name("ApiAgent")
        .spawn(async move {
            match clientgrpcagent::agent::main(
                &api_agent_task_settings,
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

    let convergance_agent_task = spawn_task(
        "ConverganceAgent",
        convergance_agent,
        &notify_shutdown,
        &shutdown_complete_tx,
    );

    let tcpcl_agent_task = spawn_task(
        "TCPCLAgent",
        tcpcl_agent,
        &notify_shutdown,
        &shutdown_complete_tx,
    );

    let node_agent_task = spawn_task(
        "NodeAgent",
        node_agent,
        &notify_shutdown,
        &shutdown_complete_tx,
    );

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
        res = convergance_agent_task => {
            if let Ok(Err(e)) = res {
                info!("something bad happened with the convergance agent {:?}. Aborting...", e);
            }
        }
        res = tcpcl_agent_task => {
            if let Ok(Err(e)) = res {
                info!("something bad happened with the tcpcl agent {:?}. Aborting...", e);
            }
        }
        res = node_agent_task => {
            if let Ok(Err(e)) = res {
                info!("something bad happened with the node agent {:?}. Aborting...", e);
            }
        }
        res = routing_agent_task => {
            if let Ok(Err(e)) = res {
                info!("something bad happened with the routing agent {:?}. Aborting...", e);
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
