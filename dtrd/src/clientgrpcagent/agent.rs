use std::task::Poll;

use futures_util::{future::FutureExt, Stream};

use bundleservice::bundle_service_server::{BundleService, BundleServiceServer};
use log::info;
use tokio::sync::{broadcast, mpsc, oneshot};
use tokio_util::sync::CancellationToken;
use tonic::{transport::Server, Response, Status};

use crate::clientagent::messages::{ClientAgentRequest, ListenBundlesResponse};
use bp7::endpoint::Endpoint;

mod bundleservice {
    tonic::include_proto!("dtn");
}

pub struct ListenBundleResponseTransformer {
    rec: mpsc::Receiver<ListenBundlesResponse>,
    canceltoken: CancellationToken,
}

impl Stream for ListenBundleResponseTransformer {
    type Item = Result<bundleservice::ListenBundleResponse, Status>;

    fn poll_next(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        match self.rec.poll_recv(cx) {
            Poll::Ready(Some(blr)) => {
                let lbr = bundleservice::ListenBundleResponse {
                    source: blr.endpoint.to_string(),
                    payload: blr.data,
                };
                Poll::Ready(Some(Ok(lbr)))
            }
            Poll::Ready(None) => Poll::Ready(None),
            Poll::Pending => Poll::Pending,
        }
    }
}

impl Drop for ListenBundleResponseTransformer {
    fn drop(&mut self) {
        self.canceltoken.cancel();
    }
}

pub struct MyBundleService {
    client_agent_sender: mpsc::Sender<ClientAgentRequest>,
}

#[tonic::async_trait]
impl BundleService for MyBundleService {
    async fn submit_bundle(
        &self,
        request: tonic::Request<bundleservice::SubmitBundleRequest>,
    ) -> Result<tonic::Response<bundleservice::SubmitBundleRespone>, tonic::Status> {
        let req = request.into_inner();

        let msg = ClientAgentRequest::ClientSendBundle {
            destination: Endpoint::new(&req.destination)
                .ok_or_else(|| tonic::Status::invalid_argument("destination invalid"))?,
            payload: req.payload,
            lifetime: req.lifetime,
        };

        self.client_agent_sender
            .send(msg)
            .await
            .map_err(|e| tonic::Status::unknown(e.to_string()))?;
        Ok(Response::new(bundleservice::SubmitBundleRespone {
            success: true,
            message: String::new(),
        }))
    }

    type ListenBundlesStream = ListenBundleResponseTransformer;
    async fn listen_bundles(
        &self,
        request: tonic::Request<bundleservice::ListenBundleRequest>,
    ) -> Result<tonic::Response<Self::ListenBundlesStream>, tonic::Status> {
        let req = request.into_inner();

        let (channel_sender, channel_receiver) = mpsc::channel(1);
        let (status_sender, status_receiver) = oneshot::channel();

        let canceltoken = CancellationToken::new();

        let msg = ClientAgentRequest::ClientListenBundles {
            destination: Endpoint::new(&req.endpoint)
                .ok_or_else(|| tonic::Status::invalid_argument("listen endpoint invalid"))?,
            responder: channel_sender,
            status: status_sender,
            canceltoken: canceltoken.clone(),
        };

        self.client_agent_sender
            .send(msg)
            .await
            .map_err(|e| tonic::Status::unknown(e.to_string()))?;

        match status_receiver.await {
            Ok(Ok(_)) => {}
            Ok(Err(msg)) => return Err(Status::invalid_argument(msg)),
            Err(_) => return Err(Status::internal("Error communicating with bpa")),
        }

        return Ok(Response::new(ListenBundleResponseTransformer {
            rec: channel_receiver,
            canceltoken,
        }));
    }

    async fn list_nodes(
        &self,
        _: tonic::Request<bundleservice::ListNodesRequest>,
    ) -> Result<tonic::Response<bundleservice::ListNodesResponse>, tonic::Status> {
        let (list_nodes_sender, list_nodes_receiver) = oneshot::channel();
        let msg = ClientAgentRequest::ClientListNodes {
            responder: list_nodes_sender,
        };

        self.client_agent_sender
            .send(msg)
            .await
            .map_err(|e| tonic::Status::unknown(e.to_string()))?;

        match list_nodes_receiver.await {
            Ok(node_list) => {
                let nodes = node_list
                    .iter()
                    .map(|node| bundleservice::Node {
                        url: node.url.clone(),
                        status: node.connection_status.to_string(),
                        endpoint: node
                            .remote_endpoint
                            .as_ref()
                            .map(|e| e.to_string())
                            .unwrap_or_else(|| "".to_string()),
                        temporary: node.temporary,
                    })
                    .collect();
                return Ok(Response::new(bundleservice::ListNodesResponse { nodes }));
            }
            Err(_) => return Err(Status::internal("Error communicating with node agent")),
        }
    }

    async fn add_node(
        &self,
        request: tonic::Request<bundleservice::AddNodeRequest>,
    ) -> Result<tonic::Response<bundleservice::AddNodeResponse>, tonic::Status> {
        let req = request.into_inner();

        let msg = ClientAgentRequest::ClientAddNode { url: req.url };

        self.client_agent_sender
            .send(msg)
            .await
            .map_err(|e| tonic::Status::unknown(e.to_string()))?;
        Ok(Response::new(bundleservice::AddNodeResponse {}))
    }

    async fn remove_node(
        &self,
        request: tonic::Request<bundleservice::RemoveNodeRequest>,
    ) -> Result<tonic::Response<bundleservice::RemoveNodeResponse>, tonic::Status> {
        let req = request.into_inner();

        let msg = ClientAgentRequest::ClientRemoveNode { url: req.url };

        self.client_agent_sender
            .send(msg)
            .await
            .map_err(|e| tonic::Status::unknown(e.to_string()))?;
        Ok(Response::new(bundleservice::RemoveNodeResponse {}))
    }
}

pub async fn main(
    mut shutdown: broadcast::Receiver<()>,
    _sender: mpsc::Sender<()>,
    client_agent_sender: mpsc::Sender<ClientAgentRequest>,
) -> Result<(), Box<dyn std::error::Error>> {
    let addr = "[::1]:50051".parse().unwrap();
    let echo = MyBundleService {
        client_agent_sender: client_agent_sender.clone(),
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
