use crate::{
    common::settings::Settings, frontend::s3::s3_frontend::S3Frontend,
    stores::storeowner::StoreOwner,
};
use actix::prelude::*;
use hyper::Server;
use log::{error, info};
use tokio::sync::{broadcast, mpsc};

use opentelemetry::KeyValue;
use opentelemetry_otlp::WithExportConfig;
use opentelemetry_sdk::{
    trace::{self, RandomIdGenerator, Sampler},
    Resource,
};
use std::time::Duration;
use tracing_subscriber::layer::SubscriberExt;

mod common;
mod frontend;
mod store;
mod stores;

fn init_tracing() {
    let tracer = opentelemetry_otlp::new_pipeline()
        .tracing()
        .with_exporter(
            opentelemetry_otlp::new_exporter()
                .tonic()
                .with_endpoint("http://localhost:4317")
                .with_timeout(Duration::from_secs(3)),
        )
        .with_trace_config(
            trace::config()
                .with_sampler(Sampler::AlwaysOn)
                .with_id_generator(RandomIdGenerator::default())
                .with_max_events_per_span(64)
                .with_max_attributes_per_span(16)
                .with_max_events_per_span(16)
                .with_resource(Resource::new(vec![KeyValue::new(
                    "service.name",
                    "replistore",
                )])),
        )
        .install_batch(opentelemetry_sdk::runtime::Tokio)
        .unwrap();

    let telemetry = tracing_opentelemetry::layer().with_tracer(tracer);

    let subscriber = tracing_subscriber::Registry::default().with(telemetry);
    tracing::subscriber::set_global_default(subscriber).unwrap();
}

#[actix_rt::main]
async fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    init_tracing();
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

    let storeowner = StoreOwner::new("/tmp/replistore/db".into())
        .unwrap()
        .start();

    let s3_addr = frontend::s3::s3::S3::new(storeowner.clone()).start();

    let s3_task_shutdown_notifier = notify_shutdown.subscribe();
    let s3_task_shutdown_complete_tx_task = shutdown_complete_tx.clone();
    let s3_task_s3_addr = s3_addr.clone();
    let s3_task = tokio::task::Builder::new()
        .name("S3")
        .spawn(async move {
            let s3 = S3Frontend::new(s3_task_s3_addr).await;
            match s3
                .run(s3_task_shutdown_notifier, s3_task_shutdown_complete_tx_task)
                .await
            {
                Ok(_) => Ok(()),
                Err(e) => Err(e.to_string()),
            }
        })
        .unwrap();

    let ctrl_c = tokio::signal::ctrl_c();

    tokio::select! {
        res = s3_task => {
            if let Ok(Err(e)) = res {
                error!("something bad happened with the s3 server {:?}. Aborting...", e);
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

    info!("Now stopping actor system");
    System::current().stop();

    // Wait for all active connections to finish processing. As the `Sender`
    // handle held by the listener has been dropped above, the only remaining
    // `Sender` instances are held by connection handler tasks. When those drop,
    // the `mpsc` channel will close and `recv()` will return `None`.
    let _ = shutdown_complete_rx.recv().await;

    info!("All done, see you");
}
