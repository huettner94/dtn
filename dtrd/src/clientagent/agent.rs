use std::collections::HashMap;

use async_trait::async_trait;
use bp7::endpoint::Endpoint;
use log::{error, info, warn};
use tokio::sync::{mpsc, oneshot};
use tokio_util::sync::CancellationToken;

use crate::{
    bundleprotocolagent::messages::BPARequest,
    common::settings::Settings,
    nodeagent::messages::{Node, NodeAgentRequest},
    routingagent::messages::{RouteStatus, RouteType, RoutingAgentRequest},
};

use super::messages::{ClientAgentRequest, ListenBundlesResponse};

pub struct Daemon {
    clients: HashMap<Endpoint, (mpsc::Sender<ListenBundlesResponse>, CancellationToken)>,
    channel_receiver: Option<mpsc::Receiver<ClientAgentRequest>>,
    bpa_sender: Option<mpsc::Sender<BPARequest>>,
    node_agent_sender: Option<mpsc::Sender<NodeAgentRequest>>,
    routing_agent_sender: Option<mpsc::Sender<RoutingAgentRequest>>,
}

#[async_trait]
impl crate::common::agent::Daemon for Daemon {
    type MessageType = ClientAgentRequest;

    fn new(_: &Settings) -> Self {
        Daemon {
            clients: HashMap::new(),
            channel_receiver: None,
            bpa_sender: None,
            node_agent_sender: None,
            routing_agent_sender: None,
        }
    }

    fn get_agent_name(&self) -> &'static str {
        "ClientAgent"
    }

    fn get_channel_receiver(&mut self) -> Option<mpsc::Receiver<Self::MessageType>> {
        self.channel_receiver.take()
    }

    async fn on_shutdown(&mut self) {
        info!("Closing all client agent channels");
        for (client_endpoint, client_sender) in self.clients.drain() {
            drop(client_sender);
            info!("Closed agent channel for {:?}", client_endpoint);
        }
    }

    async fn handle_message(&mut self, msg: ClientAgentRequest) {
        match msg {
            ClientAgentRequest::ClientSendBundle {
                destination,
                payload,
                lifetime,
            } => {
                self.message_client_send_bundle(destination, payload, lifetime)
                    .await;
            }
            ClientAgentRequest::ClientListenBundles {
                destination,
                responder,
                status,
                canceltoken,
            } => {
                self.message_client_listen_bundles(destination, responder, status, canceltoken)
                    .await;
            }
            ClientAgentRequest::ClientListNodes { responder } => {
                self.message_client_list_nodes(responder).await;
            }
            ClientAgentRequest::ClientAddNode { url } => {
                self.message_client_add_node(url).await;
            }
            ClientAgentRequest::ClientRemoveNode { url } => {
                self.message_client_remove_node(url).await;
            }
            ClientAgentRequest::ClientListRoutes { responder } => {
                self.message_client_list_routes(responder).await;
            }
            ClientAgentRequest::ClientAddRoute { target, next_hop } => {
                self.message_client_add_route(target, next_hop).await;
            }
            ClientAgentRequest::ClientRemoveRoute { target, next_hop } => {
                self.message_client_remove_route(target, next_hop).await;
            }
            ClientAgentRequest::AgentGetClient {
                destination,
                responder,
            } => {
                self.message_agent_get_client(destination, responder).await;
            }
        }
    }
}

impl Daemon {
    pub fn init_channels(
        &mut self,
        bpa_sender: tokio::sync::mpsc::Sender<BPARequest>,
        node_agent_sender: mpsc::Sender<NodeAgentRequest>,
        routing_agent_sender: mpsc::Sender<RoutingAgentRequest>,
    ) -> mpsc::Sender<ClientAgentRequest> {
        self.bpa_sender = Some(bpa_sender);
        self.node_agent_sender = Some(node_agent_sender);
        self.routing_agent_sender = Some(routing_agent_sender);
        let (channel_sender, channel_receiver) = mpsc::channel::<ClientAgentRequest>(1);
        self.channel_receiver = Some(channel_receiver);
        return channel_sender;
    }

    async fn message_client_send_bundle(
        &self,
        destination: Endpoint,
        payload: Vec<u8>,
        lifetime: u64,
    ) {
        let sender = self.bpa_sender.as_ref().unwrap();
        if let Err(e) = sender
            .send(BPARequest::SendBundle {
                destination,
                payload,
                lifetime,
            })
            .await
        {
            error!("Error sending bundle send request to BPA: {:?}", e);
        }
    }

    async fn message_client_listen_bundles(
        &mut self,
        destination: Endpoint,
        responder: mpsc::Sender<ListenBundlesResponse>,
        status: oneshot::Sender<Result<(), String>>,
        canceltoken: CancellationToken,
    ) {
        let sender = self.bpa_sender.as_ref().unwrap();
        let (endpoint_local_response_sender, endpoint_local_response_receiver) =
            oneshot::channel::<bool>();
        if let Err(e) = sender
            .send(BPARequest::IsEndpointLocal {
                endpoint: destination.clone(),
                sender: endpoint_local_response_sender,
            })
            .await
        {
            error!("Error sending is_endpoint_local to BPA: {:?}", e);
            if let Err(e) = status.send(Err("internal error".to_string())) {
                error!("Error sending response to requestor {:?}", e);
            }
            return;
        }

        match endpoint_local_response_receiver.await {
            Ok(true) => {}
            Ok(false) => {
                warn!("User attempted to register with endpoint not bound here.");
                if let Err(e) = status.send(Err(
                    "Endpoint invalid for this BundleProtocolAgent".to_string()
                )) {
                    error!("Error sending response to requestor {:?}", e);
                }
                return;
            }
            Err(e) => {
                error!("Error receiving is_endpoint_local from BPA: {:?}", e);
                if let Err(e) = status.send(Err("internal error".to_string())) {
                    error!("Error sending response to requestor {:?}", e);
                }
                return;
            }
        }

        self.clients
            .insert(destination.clone(), (responder.clone(), canceltoken));
        if let Err(e) = sender
            .send(BPARequest::NewClientConnected { destination })
            .await
        {
            error!("Error sending bundle send request to BPA: {:?}", e);
        }

        if let Err(e) = status.send(Ok(())) {
            error!("Error sending response to requestor {:?}", e);
        }
    }

    async fn message_client_list_nodes(&self, responder: oneshot::Sender<Vec<Node>>) {
        if let Err(e) = self
            .node_agent_sender
            .as_ref()
            .unwrap()
            .send(NodeAgentRequest::ListNodes { responder })
            .await
        {
            error!("Error sending request to node agent {:?}", e);
        }
    }

    async fn message_client_add_node(&self, url: String) {
        if let Err(e) = self
            .node_agent_sender
            .as_ref()
            .unwrap()
            .send(NodeAgentRequest::AddNode { url })
            .await
        {
            error!("Error sending request to node agent {:?}", e);
        }
    }

    async fn message_client_remove_node(&self, url: String) {
        if let Err(e) = self
            .node_agent_sender
            .as_ref()
            .unwrap()
            .send(NodeAgentRequest::RemoveNode { url })
            .await
        {
            error!("Error sending request to node agent {:?}", e);
        }
    }

    async fn message_client_list_routes(&self, responder: oneshot::Sender<Vec<RouteStatus>>) {
        if let Err(e) = self
            .routing_agent_sender
            .as_ref()
            .unwrap()
            .send(RoutingAgentRequest::ListRoutes { responder })
            .await
        {
            error!("Error sending request to route agent {:?}", e);
        }
    }

    async fn message_client_add_route(&self, target: Endpoint, next_hop: Endpoint) {
        if let Err(e) = self
            .routing_agent_sender
            .as_ref()
            .unwrap()
            .send(RoutingAgentRequest::AddRoute {
                target,
                next_hop,
                route_type: RouteType::Static,
            })
            .await
        {
            error!("Error sending request to route agent {:?}", e);
        }
    }

    async fn message_client_remove_route(&self, target: Endpoint, next_hop: Endpoint) {
        if let Err(e) = self
            .routing_agent_sender
            .as_ref()
            .unwrap()
            .send(RoutingAgentRequest::RemoveRoute {
                target,
                next_hop,
                route_type: RouteType::Static,
            })
            .await
        {
            error!("Error sending request to route agent {:?}", e);
        }
    }

    async fn message_agent_get_client(
        &mut self,
        destination: Endpoint,
        responder: oneshot::Sender<Option<mpsc::Sender<ListenBundlesResponse>>>,
    ) {
        let resp = match self.clients.get(&destination) {
            Some((sender, canceltoken)) => {
                if canceltoken.is_cancelled() {
                    info!("Client for endpoint {} already disconnected", destination);
                    self.clients.remove(&destination);
                    None
                } else {
                    Some(sender.clone())
                }
            }
            None => None,
        };
        if let Err(e) = responder.send(resp) {
            warn!("Error sending client get back to requestor: {:?}", e);
        }
    }
}
