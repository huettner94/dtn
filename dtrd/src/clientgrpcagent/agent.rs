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

use std::task::Poll;

use actix::Addr;
use futures_util::{future::FutureExt, Stream};

use adminservice::admin_service_server::{AdminService, AdminServiceServer};
use bundleservice::bundle_service_server::{BundleService, BundleServiceServer};
use log::info;
use tokio::sync::{broadcast, mpsc};
use tonic::{transport::Server, Response, Status};
use url::Url;

use crate::{
    clientagent::{
        self,
        messages::{
            ClientAddNode, ClientAddRoute, ClientDeliverBundle, ClientListNodes, ClientListRoutes,
            ClientListenConnect, ClientListenDisconnect, ClientRemoveNode, ClientRemoveRoute,
            ClientSendBundle, EventBundleDelivered,
        },
    },
    common::settings::Settings,
    routingagent::messages::RouteType,
};
use bp7::endpoint::Endpoint;

mod bundleservice {
    tonic::include_proto!("dtn_bundle");
}

mod adminservice {
    tonic::include_proto!("dtn_admin");
}

pub struct ListenBundleResponseTransformer {
    client_agent: Addr<clientagent::agent::Daemon>,
    destination: Endpoint,
    rec: mpsc::Receiver<ClientDeliverBundle>,
}

impl Stream for ListenBundleResponseTransformer {
    type Item = Result<bundleservice::ListenBundleResponse, Status>;

    fn poll_next(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        match self.rec.poll_recv(cx) {
            Poll::Ready(Some(cdb)) => {
                let lbr = bundleservice::ListenBundleResponse {
                    source: cdb
                        .bundle
                        .get_bundle()
                        .primary_block
                        .source_node
                        .to_string(),
                    payload: cdb.bundle.get_bundle().payload_block().data.clone(), //TODO: this seems heavy
                };
                cdb.responder.do_send(EventBundleDelivered {
                    endpoint: cdb
                        .bundle
                        .get_bundle()
                        .primary_block
                        .destination_endpoint
                        .clone(),
                    bundle: cdb.bundle.clone(),
                });
                Poll::Ready(Some(Ok(lbr)))
            }
            Poll::Ready(None) => Poll::Ready(None),
            Poll::Pending => Poll::Pending,
        }
    }
}

impl Drop for ListenBundleResponseTransformer {
    fn drop(&mut self) {
        self.client_agent.do_send(ClientListenDisconnect {
            destination: self.destination.clone(),
        });
    }
}

pub struct MyBundleService {
    client_agent: Addr<clientagent::agent::Daemon>,
}

#[tonic::async_trait]
impl BundleService for MyBundleService {
    async fn submit_bundle(
        &self,
        request: tonic::Request<bundleservice::SubmitBundleRequest>,
    ) -> Result<tonic::Response<bundleservice::SubmitBundleRespone>, tonic::Status> {
        let req = request.into_inner();
        let destination = Endpoint::new(&req.destination)
            .ok_or_else(|| tonic::Status::invalid_argument("destination invalid"))?;

        let send_result = self
            .client_agent
            .send(ClientSendBundle {
                destination,
                payload: req.payload,
                lifetime: req.lifetime,
            })
            .await
            .map_err(|e| tonic::Status::unknown(e.to_string()))?;

        match send_result {
            Ok(_) => Ok(Response::new(bundleservice::SubmitBundleRespone {
                success: true,
                message: String::new(),
            })),
            Err(_) => Err(tonic::Status::internal(
                "something prevented the bundle from being accepted",
            )),
        }
    }

    type ListenBundlesStream = ListenBundleResponseTransformer;
    async fn listen_bundles(
        &self,
        request: tonic::Request<bundleservice::ListenBundleRequest>,
    ) -> Result<tonic::Response<Self::ListenBundlesStream>, tonic::Status> {
        let req = request.into_inner();
        let destination = Endpoint::new(&req.endpoint)
            .ok_or_else(|| tonic::Status::invalid_argument("destination invalid"))?;

        let (sender, receiver) = mpsc::channel(1);

        let result = self
            .client_agent
            .send(ClientListenConnect {
                destination: destination.clone(),
                sender,
            })
            .await
            .map_err(|e| tonic::Status::unknown(e.to_string()))?;

        match result {
            Ok(_) => {
                let response_transformer = ListenBundleResponseTransformer {
                    client_agent: self.client_agent.clone(),
                    destination,
                    rec: receiver,
                };
                Ok(Response::new(response_transformer))
            }
            Err(msg) => Err(tonic::Status::invalid_argument(msg)),
        }
    }
}

pub struct MyAdminService {
    client_agent: Addr<clientagent::agent::Daemon>,
}

#[tonic::async_trait]
impl AdminService for MyAdminService {
    async fn list_nodes(
        &self,
        _: tonic::Request<adminservice::ListNodesRequest>,
    ) -> Result<tonic::Response<adminservice::ListNodesResponse>, tonic::Status> {
        let node_list = self
            .client_agent
            .send(ClientListNodes {})
            .await
            .map_err(|e| tonic::Status::unknown(e.to_string()))?;

        let nodes = node_list
            .iter()
            .map(|node| adminservice::Node {
                url: node.url.to_string(),
                status: node.connection_status.to_string(),
                endpoint: node
                    .remote_endpoint
                    .as_ref()
                    .map(|e| e.to_string())
                    .unwrap_or_else(|| "".to_string()),
                temporary: node.temporary,
            })
            .collect();
        return Ok(Response::new(adminservice::ListNodesResponse { nodes }));
    }

    async fn add_node(
        &self,
        request: tonic::Request<adminservice::AddNodeRequest>,
    ) -> Result<tonic::Response<adminservice::AddNodeResponse>, tonic::Status> {
        let req = request.into_inner();
        let url = Url::parse(&req.url).map_err(|e| Status::invalid_argument(e.to_string()))?;
        self.client_agent
            .send(ClientAddNode { url })
            .await
            .map_err(|e| tonic::Status::unknown(e.to_string()))?;
        Ok(Response::new(adminservice::AddNodeResponse {}))
    }

    async fn remove_node(
        &self,
        request: tonic::Request<adminservice::RemoveNodeRequest>,
    ) -> Result<tonic::Response<adminservice::RemoveNodeResponse>, tonic::Status> {
        let req = request.into_inner();
        let url = Url::parse(&req.url).map_err(|e| Status::invalid_argument(e.to_string()))?;
        self.client_agent
            .send(ClientRemoveNode { url })
            .await
            .map_err(|e| tonic::Status::unknown(e.to_string()))?;
        Ok(Response::new(adminservice::RemoveNodeResponse {}))
    }

    async fn list_routes(
        &self,
        _: tonic::Request<adminservice::ListRoutesRequest>,
    ) -> Result<tonic::Response<adminservice::ListRoutesResponse>, tonic::Status> {
        let route_list = self
            .client_agent
            .send(ClientListRoutes {})
            .await
            .map_err(|e| tonic::Status::unknown(e.to_string()))?;

        let routes = route_list
            .iter()
            .map(|route| {
                let route_type = match route.route_type {
                    RouteType::Connected => 0,
                    RouteType::Static => 1,
                };
                adminservice::RouteStatus {
                    route: Some(adminservice::Route {
                        target: route.target.to_string(),
                        next_hop: route.next_hop.to_string(),
                    }),
                    r#type: route_type,
                    preferred: route.preferred,
                    available: route.available,
                    max_bundle_size: route.max_bundle_size.unwrap_or(0),
                }
            })
            .collect();
        return Ok(Response::new(adminservice::ListRoutesResponse { routes }));
    }

    async fn add_route(
        &self,
        request: tonic::Request<adminservice::AddRouteRequest>,
    ) -> Result<tonic::Response<adminservice::AddRouteResponse>, tonic::Status> {
        let req = request.into_inner();

        let route = req
            .route
            .ok_or_else(|| tonic::Status::invalid_argument("Route must be set"))?;

        let target = Endpoint::new(&route.target)
            .ok_or_else(|| tonic::Status::invalid_argument("target invalid"))?;
        let next_hop = Endpoint::new(&route.next_hop)
            .ok_or_else(|| tonic::Status::invalid_argument("next_hop invalid"))?;

        self.client_agent
            .send(ClientAddRoute { target, next_hop })
            .await
            .map_err(|e| tonic::Status::unknown(e.to_string()))?;
        Ok(Response::new(adminservice::AddRouteResponse {}))
    }

    async fn remove_route(
        &self,
        request: tonic::Request<adminservice::RemoveRouteRequest>,
    ) -> Result<tonic::Response<adminservice::RemoveRouteResponse>, tonic::Status> {
        let req = request.into_inner();

        let route = req
            .route
            .ok_or_else(|| tonic::Status::invalid_argument("Route must be set"))?;

        let target = Endpoint::new(&route.target)
            .ok_or_else(|| tonic::Status::invalid_argument("target invalid"))?;
        let next_hop = Endpoint::new(&route.next_hop)
            .ok_or_else(|| tonic::Status::invalid_argument("next_hop invalid"))?;

        self.client_agent
            .send(ClientRemoveRoute { target, next_hop })
            .await
            .map_err(|e| tonic::Status::unknown(e.to_string()))?;
        Ok(Response::new(adminservice::RemoveRouteResponse {}))
    }
}

pub async fn main(
    mut shutdown: broadcast::Receiver<()>,
    _shutdown_complete_sender: mpsc::Sender<()>,
    client_agent: Addr<clientagent::agent::Daemon>,
) -> Result<(), Box<dyn std::error::Error>> {
    let settings = Settings::from_env();
    let addr = settings.grpc_clientapi_address.parse().unwrap();
    let bundle_service = MyBundleService {
        client_agent: client_agent.clone(),
    };
    let admin_service = MyAdminService {
        client_agent: client_agent.clone(),
    };

    info!("Server listening on {}", addr);
    const MESSAGE_SIZE_LIMIT: usize = 10 * 1024 * 1024 * 1024;
    Server::builder()
        .add_service(
            BundleServiceServer::new(bundle_service)
                .max_decoding_message_size(MESSAGE_SIZE_LIMIT)
                .max_encoding_message_size(MESSAGE_SIZE_LIMIT),
        )
        .add_service(AdminServiceServer::new(admin_service))
        .serve_with_shutdown(addr, shutdown.recv().map(|_| ()))
        .await?;

    info!("Server has shutdown. See you");
    // _shutdown_complete_sender is explicitly dropped here
    Ok(())
}
