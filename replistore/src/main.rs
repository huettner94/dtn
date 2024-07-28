// Copyright (C) 2023 Felix Huettner
//
// This file is part of DTRD.
//
// DTRD is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// DTRD is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with this program.  If not, see <https://www.gnu.org/licenses/>.

use crate::{
    common::settings::Settings, frontend::s3::s3_frontend::S3Frontend,
    stores::storeowner::StoreOwner,
};
use actix::prelude::*;
use log::{error, info};
use tokio::sync::{broadcast, mpsc};

use opentelemetry::trace::TracerProvider;
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
mod stores;

fn init_tracing(settings: &Settings) {
    let tracerprovider = opentelemetry_otlp::new_pipeline()
        .tracing()
        .with_exporter(
            opentelemetry_otlp::new_exporter()
                .tonic()
                .with_endpoint("http://localhost:4317")
                .with_timeout(Duration::from_secs(3)),
        )
        .with_trace_config(
            trace::Config::default()
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

    let tracer = tracerprovider.tracer("replistore");
    let telemetry = tracing_opentelemetry::layer().with_tracer(tracer);

    let console_layer = console_subscriber::ConsoleLayer::builder()
        .server_addr((
            [127, 0, 0, 1],
            settings
                .tokio_tracing_port
                .clone()
                .map_or(0, |e| e.parse().unwrap()),
        ))
        .spawn();

    let subscriber = tracing_subscriber::Registry::default()
        .with(telemetry)
        .with(console_layer);

    tracing::subscriber::set_global_default(subscriber).unwrap();
}

#[actix_rt::main]
async fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    info!("Starting up");
    let settings: Settings = Settings::from_env();
    info!("Starting with settings: {:?}", settings);
    init_tracing(&settings);

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
