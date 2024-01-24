use crate::common::settings::Settings;
use actix::System;
use hyper::Server;
use log::info;
use s3s::{auth::SimpleAuth, service::S3ServiceBuilder};
use tokio::sync::{broadcast, mpsc};

mod bitswap;
mod common;
mod s3;
mod store;

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

    let store = store::Store::new("/tmp/replistore");
    store.load().await.unwrap();

    let fs = s3::FileStore::new(store);

    // Setup S3 service
    let service = {
        let mut b = S3ServiceBuilder::new(fs);

        // Enable authentication
        b.set_auth(SimpleAuth::from_single("cake", "ilike"));

        b.build()
    };

    // Run server
    let addr = "0.0.0.0:8080".parse().unwrap();
    let server = Server::try_bind(&addr)
        .unwrap()
        .serve(service.into_shared().into_make_service());

    info!("server is running at http://{addr}");
    server
        .with_graceful_shutdown(shutdown_signal())
        .await
        .unwrap();

    info!("Stopping external connections");
    // Stolen from: https://github.com/tokio-rs/mini-redis/blob/master/src/server.rs
    // When `notify_shutdown` is dropped, all tasks which have `subscribe`d will
    // receive the shutdown signal and can exit
    drop(notify_shutdown);
    // Drop final `Sender` so the `Receiver` below can complete
    drop(shutdown_complete_tx);

    info!("Stopping individual actors");

    info!("Now stopping actor system");
    System::current().stop();

    // Wait for all active connections to finish processing. As the `Sender`
    // handle held by the listener has been dropped above, the only remaining
    // `Sender` instances are held by connection handler tasks. When those drop,
    // the `mpsc` channel will close and `recv()` will return `None`.
    let _ = shutdown_complete_rx.recv().await;

    info!("All done, see you");
}

async fn shutdown_signal() {
    let _ = tokio::signal::ctrl_c().await;
}
