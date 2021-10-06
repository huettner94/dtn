use std::collections::HashMap;

use async_trait::async_trait;
use bp7::endpoint::Endpoint;
use log::{error, info, warn};
use tokio::sync::{mpsc, oneshot};

use crate::{bundleprotocolagent::messages::BPARequest, common::canceltoken::CancelToken};

use super::messages::{ClientAgentRequest, ListenBundlesResponse};

pub struct Daemon {
    clients: HashMap<Endpoint, (mpsc::Sender<ListenBundlesResponse>, CancelToken)>,
    channel_receiver: Option<mpsc::Receiver<ClientAgentRequest>>,
    bpa_sender: Option<mpsc::Sender<BPARequest>>,
}

#[async_trait]
impl crate::common::agent::Daemon for Daemon {
    type MessageType = ClientAgentRequest;

    fn new() -> Self {
        Daemon {
            clients: HashMap::new(),
            channel_receiver: None,
            bpa_sender: None,
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
            ClientAgentRequest::ClientListenBundles {
                destination,
                responder,
                status,
                canceltoken,
            } => {
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
                            "Endpoint invalid for this BundleProtocolAgent".to_string(),
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
            ClientAgentRequest::AgentGetClient {
                destination,
                responder,
            } => {
                let resp = match self.clients.get(&destination) {
                    Some((sender, canceltoken)) => {
                        if canceltoken.is_canceled() {
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
    }
}

impl Daemon {
    pub fn init_channels(
        &mut self,
        bpa_sender: tokio::sync::mpsc::Sender<BPARequest>,
    ) -> mpsc::Sender<ClientAgentRequest> {
        self.bpa_sender = Some(bpa_sender);
        let (channel_sender, channel_receiver) = mpsc::channel::<ClientAgentRequest>(1);
        self.channel_receiver = Some(channel_receiver);
        return channel_sender;
    }
}
