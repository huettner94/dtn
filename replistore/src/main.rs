use hyper::Server;
use log::info;
use s3s::{auth::SimpleAuth, service::S3ServiceBuilder};

mod bitswap;
mod s3;
mod store;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    info!("Starting up");

    // let client = dtrd_client::Client::new("http://localhost:50051").await?;
    // let server = bitswap::BitswapServer::new(client);
    // server.run().await?;

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
    let server = Server::try_bind(&addr)?.serve(service.into_shared().into_make_service());

    info!("server is running at http://{addr}");
    server.with_graceful_shutdown(shutdown_signal()).await?;

    Ok(())
}

async fn shutdown_signal() {
    let _ = tokio::signal::ctrl_c().await;
}
