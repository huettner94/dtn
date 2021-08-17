use futures_util::future::FutureExt;

use bundleservice::bundle_service_server::{BundleService, BundleServiceServer};
use log::info;
use tokio::sync::{broadcast, mpsc};
use tonic::{transport::Server, Response};

mod bundleservice {
    tonic::include_proto!("dtn");
}

// defining a struct for our service
#[derive(Default)]
pub struct MySay {}

// implementing rpc for service defined in .proto
#[tonic::async_trait]
impl BundleService for MySay {
    async fn submit_bundle(
        &self,
        request: tonic::Request<bundleservice::SubmitBundleRequest>,
    ) -> Result<tonic::Response<bundleservice::SubmitBundleRespone>, tonic::Status> {
        Ok(Response::new(bundleservice::SubmitBundleRespone {
            success: true,
            message: request.into_inner().message,
        }))
    }
}

pub async fn main(
    mut shutdown: broadcast::Receiver<()>,
    _sender: mpsc::Sender<()>,
) -> Result<(), Box<dyn std::error::Error>> {
    // defining address for our service
    let addr = "[::1]:50051".parse().unwrap();
    // creating a service
    let echo = MySay::default();

    info!("Server listening on {}", addr);
    // adding our service to our server.
    Server::builder()
        .add_service(BundleServiceServer::new(echo))
        .serve_with_shutdown(addr, shutdown.recv().map(|_| ()))
        .await?;
    info!("Server has shutdown. See you");
    // _sender is explicitly dropped here
    Ok(())
}
