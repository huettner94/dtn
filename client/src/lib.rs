use std::str::FromStr;

use crate::error::Error;
use adminservice::admin_service_client::AdminServiceClient;
use bundleservice::bundle_service_client::BundleServiceClient;
use futures_util::Stream;
use futures_util::StreamExt;
use maybe_async::maybe_async;
use tonic::transport::{Channel, Uri};

mod bundleservice {
    tonic::include_proto!("dtn_bundle");
}

mod adminservice {
    tonic::include_proto!("dtn_admin");
}

pub mod error;

pub struct Client {
    admin_client: AdminServiceClient<Channel>,
    bundle_client: BundleServiceClient<Channel>,
}

impl Clone for Client {
    fn clone(&self) -> Self {
        Client {
            admin_client: self.admin_client.clone(),
            bundle_client: self.bundle_client.clone(),
        }
    }
}

impl Client {
    #[maybe_async]
    pub async fn new(url: &str) -> Result<Self, Error> {
        let uri = Uri::from_str(url)?;
        let channel = Channel::builder(uri).connect().await?;
        let admin_client = AdminServiceClient::new(channel.clone());
        let bundle_client = BundleServiceClient::new(channel);
        Ok(Self {
            admin_client,
            bundle_client,
        })
    }

    #[maybe_async]
    pub async fn submit_bundle(
        &mut self,
        target: &str,
        lifetime: u64,
        data: &[u8],
    ) -> Result<(), Error> {
        let req = bundleservice::SubmitBundleRequest {
            destination: target.to_string(),
            lifetime,
            payload: data.to_vec(),
        };
        self.bundle_client.submit_bundle(req).await?;
        Ok(())
    }

    #[maybe_async]
    pub async fn listen_bundles(
        &mut self,
        endpoint: &str,
    ) -> Result<impl Stream<Item = Result<Vec<u8>, Error>>, Error> {
        let req = bundleservice::ListenBundleRequest {
            endpoint: endpoint.to_string(),
        };
        let stream = self
            .bundle_client
            .listen_bundles(req)
            .await?
            .into_inner()
            .map(|r| match r {
                Ok(b) => Ok(b.payload),
                Err(e) => Err(Error::GrpcError(e)),
            });
        Ok(stream)
    }
}
