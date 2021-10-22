use async_trait::async_trait;
use bp7::{
    block::payload_block::PayloadBlock,
    block::{Block, CanonicalBlock},
    blockflags::BlockFlags,
    bundle::Bundle,
    bundleflags::BundleFlags,
    crc::CRCType,
    endpoint::Endpoint,
    primaryblock::PrimaryBlock,
    time::{CreationTimestamp, DtnTime},
};
use log::{debug, error, info};
use tokio::sync::{mpsc, oneshot};

use crate::{
    bundlestorageagent::messages::BSARequest,
    clientagent::messages::{ClientAgentRequest, ListenBundlesResponse},
    converganceagent::messages::{AgentForwardBundle, ConverganceAgentRequest},
};

use super::messages::BPARequest;

pub struct Daemon {
    endpoint: Endpoint,
    channel_receiver: Option<mpsc::Receiver<BPARequest>>,
    bsa_sender: Option<mpsc::Sender<BSARequest>>,
    client_agent_sender: Option<mpsc::Sender<ClientAgentRequest>>,
    convergance_agent_sender: Option<mpsc::Sender<ConverganceAgentRequest>>,
}

#[async_trait]
impl crate::common::agent::Daemon for Daemon {
    type MessageType = BPARequest;

    fn new() -> Self {
        Daemon {
            endpoint: Endpoint::new(&"dtn://itsme").unwrap(),
            channel_receiver: None,
            bsa_sender: None,
            client_agent_sender: None,
            convergance_agent_sender: None,
        }
    }

    fn get_agent_name(&self) -> &'static str {
        "BPA"
    }

    fn get_channel_receiver(&mut self) -> Option<mpsc::Receiver<Self::MessageType>> {
        self.channel_receiver.take()
    }

    fn validate(&self) {
        if self.client_agent_sender.is_none() {
            panic!(
                "Must call set_client_agent before calling run (also run may only be called once)"
            );
        }
    }

    async fn handle_message(&mut self, msg: BPARequest) {
        match msg {
            BPARequest::SendBundle {
                destination,
                payload,
                lifetime,
            } => {
                self.message_send_bundle(destination, payload, lifetime)
                    .await;
            }
            BPARequest::IsEndpointLocal { endpoint, sender } => {
                self.message_is_endpoint_local(endpoint, sender).await;
            }
            BPARequest::NewClientConnected { destination } => {
                self.message_new_client_connected(destination).await;
            }
        }
    }
}

impl Daemon {
    pub fn init_channels(
        &mut self,
        bsa_sender: tokio::sync::mpsc::Sender<BSARequest>,
    ) -> mpsc::Sender<BPARequest> {
        self.bsa_sender = Some(bsa_sender);
        let (channel_sender, channel_receiver) = mpsc::channel::<BPARequest>(1);
        self.channel_receiver = Some(channel_receiver);
        return channel_sender;
    }

    pub fn set_agents(
        &mut self,
        client_agent_sender: mpsc::Sender<ClientAgentRequest>,
        convergance_agent_sender: mpsc::Sender<ConverganceAgentRequest>,
    ) {
        self.client_agent_sender = Some(client_agent_sender);
        self.convergance_agent_sender = Some(convergance_agent_sender);
    }

    async fn message_send_bundle(&self, destination: Endpoint, payload: Vec<u8>, lifetime: u64) {
        self.transmit_bundle(destination, payload, lifetime).await;
    }

    async fn message_is_endpoint_local(&self, endpoint: Endpoint, sender: oneshot::Sender<bool>) {
        if let Err(e) = sender.send(self.endpoint.matches_node(&endpoint)) {
            error!("Error sending response to requestor {:?}", e);
        }
    }

    async fn message_new_client_connected(&self, destination: Endpoint) {
        let (response_sender, response_receiver) = oneshot::channel();
        if let Err(e) = self
            .bsa_sender
            .as_ref()
            .unwrap()
            .send(BSARequest::GetBundleForDestination {
                destination,
                bundles: response_sender,
            })
            .await
        {
            error!("Error sending request to bsa {:?}", e);
        };

        match response_receiver.await {
            Ok(Ok(bundles)) => {
                for bundle in bundles {
                    self.dispatch_bundle(bundle).await;
                }
            }
            Ok(Err(e)) => {
                error!("Error receiving response from bsa {:?}", e);
            }
            Err(e) => {
                error!("Error receiving response from bsa {:?}", e);
            }
        }
    }

    async fn transmit_bundle(&self, destination: Endpoint, data: Vec<u8>, lifetime: u64) {
        let bundle = Bundle {
            primary_block: PrimaryBlock {
                version: 7,
                bundle_processing_flags: BundleFlags::empty(),
                crc: CRCType::NoCRC,
                destination_endpoint: destination,
                source_node: self.endpoint.clone(),
                report_to: self.endpoint.clone(),
                creation_timestamp: CreationTimestamp {
                    creation_time: DtnTime::now(),
                    sequence_number: 0, // TODO: Needs to increase for all of the same timestamp
                },
                lifetime,
                fragment_offset: None,
                total_data_length: None,
            },
            blocks: vec![CanonicalBlock {
                block: Block::Payload(PayloadBlock { data }),
                block_flags: BlockFlags::empty(),
                block_number: 1,
                crc: CRCType::NoCRC,
            }],
        };
        debug!("Dispatching new bundle {:?}", &bundle);
        self.dispatch_bundle(bundle).await;
    }

    async fn dispatch_bundle(&self, bundle: Bundle) {
        if bundle
            .primary_block
            .destination_endpoint
            .matches_node(&self.endpoint)
        {
            match self.local_delivery(&bundle).await {
                Ok(_) => {}
                Err(_) => {
                    info!("Some issue appeared during local delivery. Adding to todo list.");
                    self.store_bundle(bundle).await;
                }
            };
        } else {
            info!("Bundle is not for me. trying to forward");
            match self.forward_bundle(bundle).await {
                Ok(_) => {}
                Err(bundle) => {
                    info!("Some issue appeared during local delivery. Adding to todo list.");
                    self.store_bundle(bundle).await;
                }
            };
        }
    }

    async fn store_bundle(&self, bundle: Bundle) {
        let sender = self.bsa_sender.as_ref().unwrap();
        if let Err(e) = sender.send(BSARequest::StoreBundle { bundle }).await {
            error!(
                "Error during sending the bundle to the BSA for storage {:?}",
                e
            );
        };
    }

    async fn forward_bundle(&self, bundle: Bundle) -> Result<(), Bundle> {
        debug!("forwarding bundle {:?}", &bundle);
        if bundle.primary_block.fragment_offset.is_some() {
            panic!("Bundle is a fragment. No idea what to do");
            //TODO
        }

        if let Some(sender) = self
            .get_connected_node(bundle.primary_block.destination_endpoint.clone())
            .await
        {
            let result = sender
                .send(AgentForwardBundle {
                    bundle: bundle.clone(),
                })
                .await;
            match result {
                Ok(_) => {
                    //TODO: send status report if reqeusted
                    debug!("Bundle forwarded to remote node");
                    return Ok(());
                }
                Err(_) => {
                    info!("Remote node not available. Bundle queued.");
                    return Err(bundle);
                }
            }
        } else {
            info!("No remote node registered for endpoint. Bundle queued.");
            return Err(bundle);
        }
    }

    async fn receive_bundle(&mut self, bundle: Bundle) {
        //TODO: send status report if reqeusted
        //TODO: Check crc or drop otherwise
        //TODO: CHeck extensions or do other stuff
        self.dispatch_bundle(bundle).await;
    }

    async fn local_delivery(&self, bundle: &Bundle) -> Result<(), ()> {
        debug!("locally delivering bundle {:?}", &bundle);
        if bundle.primary_block.fragment_offset.is_some() {
            panic!("Bundle is a fragment. No idea what to do");
            //TODO
        }

        if let Some(sender) = self
            .get_connected_client(bundle.primary_block.destination_endpoint.clone())
            .await
        {
            let result = sender
                .send(ListenBundlesResponse {
                    data: bundle.payload_block().data,
                    endpoint: bundle.primary_block.source_node.clone(),
                })
                .await;
            match result {
                Ok(_) => {
                    //TODO: send status report if reqeusted
                    debug!("Bundle dispatched to local agent");
                    Ok(())
                }
                Err(_) => {
                    info!("Local agent not available. Bundle queued.");
                    Err(())
                }
            }
        } else {
            info!("No local agent registered for endpoint. Bundle queued.");
            Err(())
        }
    }

    async fn get_connected_client(
        &self,
        endpoint: Endpoint,
    ) -> Option<mpsc::Sender<ListenBundlesResponse>> {
        let (responder_sender, responder_receiver) =
            oneshot::channel::<Option<mpsc::Sender<ListenBundlesResponse>>>();

        let client_agent = self.client_agent_sender.as_ref().unwrap();
        if let Err(e) = client_agent
            .send(ClientAgentRequest::AgentGetClient {
                destination: endpoint,
                responder: responder_sender,
            })
            .await
        {
            error!("Error sending request to Client Agent: {:?}", e);
            return None;
        }

        match responder_receiver.await {
            Ok(s) => s,
            Err(e) => {
                error!("Error receiving response from Client Agent: {:?}", e);
                None
            }
        }
    }

    async fn get_connected_node(
        &self,
        endpoint: Endpoint,
    ) -> Option<mpsc::Sender<AgentForwardBundle>> {
        let (responder_sender, responder_receiver) =
            oneshot::channel::<Option<mpsc::Sender<AgentForwardBundle>>>();

        let convergance_agent = self.convergance_agent_sender.as_ref().unwrap();
        if let Err(e) = convergance_agent
            .send(ConverganceAgentRequest::AgentGetNode {
                destination: endpoint,
                responder: responder_sender,
            })
            .await
        {
            error!("Error sending request to Convergance Agent: {:?}", e);
            return None;
        }

        match responder_receiver.await {
            Ok(s) => s,
            Err(e) => {
                error!("Error receiving response from Convergance Agent: {:?}", e);
                None
            }
        }
    }
}
