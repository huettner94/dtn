use futures_util::future::FutureExt;

use bundleservice::bundle_service_server::{BundleService, BundleServiceServer};
use log::info;
use tokio::sync::{broadcast, mpsc};
use tonic::{transport::Server, Response};

mod bundleservice {
    tonic::include_proto!("dtn");
}

pub struct MyBundleService {
    bpa_sender: mpsc::Sender<()>,
}

#[tonic::async_trait]
impl BundleService for MyBundleService {
    async fn submit_bundle(
        &self,
        request: tonic::Request<bundleservice::SubmitBundleRequest>,
    ) -> Result<tonic::Response<bundleservice::SubmitBundleRespone>, tonic::Status> {
        self.bpa_sender
            .send(())
            .await
            .map_err(|e| tonic::Status::unknown(e.to_string()))?;
        Ok(Response::new(bundleservice::SubmitBundleRespone {
            success: true,
            message: request.into_inner().message,
        }))
    }
}

pub async fn main(
    mut shutdown: broadcast::Receiver<()>,
    _sender: mpsc::Sender<()>,
    bpa_sender: mpsc::Sender<()>,
) -> Result<(), Box<dyn std::error::Error>> {
    let addr = "[::1]:50051".parse().unwrap();
    let echo = MyBundleService {
        bpa_sender: bpa_sender.clone(),
    };

    info!("Server listening on {}", addr);
    Server::builder()
        .add_service(BundleServiceServer::new(echo))
        .serve_with_shutdown(addr, shutdown.recv().map(|_| ()))
        .await?;
    info!("Server has shutdown. See you");
    // _sender is explicitly dropped here
    Ok(())
}
