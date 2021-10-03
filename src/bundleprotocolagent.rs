use dtn::bp7::{
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
use log::{debug, info, warn};
use std::collections::HashMap;
use tokio::sync::{broadcast, mpsc};

use crate::shutdown::Shutdown;

#[derive(Debug, PartialEq, Eq)]
enum BundleConstraint {
    DispatchPending,
    ForwardPending,
}

#[derive(Debug)]
struct BundleProcessing {
    bundle: Bundle,
    bundle_constraint: Option<BundleConstraint>,
}

#[derive(Debug, PartialEq, Eq)]
pub struct BundleListenResponse {
    pub endpoint: Endpoint,
    pub data: Vec<u8>,
}

#[derive(Debug)]
pub enum BPAMessage {
    // Destination, Payload, Lifetime
    SendBundle(Endpoint, Vec<u8>, u64),
    // destination, Responder
    ListenBundles(Endpoint, mpsc::Sender<BundleListenResponse>),
}

pub struct Daemon {
    todo: Vec<BundleProcessing>,
    endpoint: Endpoint,
    channel_receiver: Option<mpsc::Receiver<BPAMessage>>,
    clients: HashMap<Endpoint, mpsc::Sender<BundleListenResponse>>,
}

impl Daemon {
    pub fn new() -> Self {
        Daemon {
            todo: Vec::new(),
            endpoint: Endpoint::new(&"dtn://itsme").unwrap(),
            channel_receiver: None,
            clients: HashMap::new(),
        }
    }

    pub fn init_channel(&mut self) -> mpsc::Sender<BPAMessage> {
        let (channel_sender, channel_receiver) = mpsc::channel::<BPAMessage>(1);
        self.channel_receiver = Some(channel_receiver);
        return channel_sender;
    }

    pub async fn run(
        mut self,
        shutdown_signal: broadcast::Receiver<()>,
        _sender: mpsc::Sender<()>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        if self.channel_receiver.is_none() {
            panic!("Must call init_cannel before calling run (also run may only be called once)");
        }
        info!("BPA starting...");

        let mut shutdown = Shutdown::new(shutdown_signal);
        let mut receiver = self.channel_receiver.take().unwrap();

        while !shutdown.is_shutdown() {
            tokio::select! {
                res = receiver.recv() => {
                    if let Some(msg) = res {
                        self.handle_message(msg).await;
                    } else {
                        info!("BPA can no longer receive messages. Exiting");
                        return Ok(())
                    }
                }
                _ = shutdown.recv() => {
                    info!("BPA received shutdown");
                    receiver.close();
                    info!("BPA will not allow more requests to be sent");
                }
            }
        }

        while let Some(msg) = receiver.recv().await {
            self.handle_message(msg).await;
        }

        info!("Closing all client agent channels");
        for (client_endpoint, client_sender) in self.clients.drain() {
            drop(client_sender);
            info!("Closed agent channel for {:?}", client_endpoint);
        }

        info!("BPA has shutdown. See you");
        // _sender is explicitly dropped here
        Ok(())
    }

    async fn handle_message(&mut self, msg: BPAMessage) {
        match msg {
            BPAMessage::SendBundle(destination, data, lifetime) => {
                self.transmit_bundle(destination, data, lifetime).await;
            }
            BPAMessage::ListenBundles(endpoint, channel) => {
                info!("Registering new client for endpoint {}", endpoint);
                self.clients.insert(endpoint, channel);
            }
        }
    }

    async fn transmit_bundle(&mut self, destination: Endpoint, data: Vec<u8>, lifetime: u64) {
        let bundle = BundleProcessing {
            bundle: Bundle {
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
            },
            bundle_constraint: Some(BundleConstraint::DispatchPending),
        };
        debug!("Dispatching new bundle {:?}", &bundle);
        self.dispatch_bundle(bundle).await;
    }

    async fn dispatch_bundle(&mut self, bundle: BundleProcessing) {
        if bundle
            .bundle
            .primary_block
            .destination_endpoint
            .matches_node(&self.endpoint)
        {
            self.local_delivery(bundle).await;
        } else {
            info!("Bundle is not for me. adding to todo list {:?}", &bundle);
            self.todo.push(bundle);
        }
    }

    fn forward_bundle(&self, mut bundle: BundleProcessing) {
        bundle.bundle_constraint = Some(BundleConstraint::ForwardPending);
        info!("No idea what to do now :)");
        //TODO
    }

    async fn receive_bundle(&mut self, mut bundle: BundleProcessing) {
        bundle.bundle_constraint = Some(BundleConstraint::DispatchPending);
        //TODO: send status report if reqeusted
        //TODO: Check crc or drop otherwise
        //TODO: CHeck extensions or do other stuff
        self.dispatch_bundle(bundle).await;
    }

    async fn local_delivery(&mut self, bundle: BundleProcessing) {
        debug!("locally delivering bundle {:?}", &bundle);
        if bundle.bundle.primary_block.fragment_offset.is_some() {
            warn!("Bundle is a fragment. No idea what to do");
            return;
        }
        //TODO: send status report if reqeusted
        if let Some(sender) = self
            .clients
            .get(&bundle.bundle.primary_block.destination_endpoint)
        {
            let result = sender
                .send(BundleListenResponse {
                    data: bundle.bundle.payload_block().data,
                    endpoint: bundle.bundle.primary_block.source_node,
                })
                .await;
            match result {
                Ok(_) => {
                    debug!("Bundle dispatched to local agent");
                }
                Err(_) => {
                    //TODO
                    self.clients
                        .remove(&bundle.bundle.primary_block.destination_endpoint);
                    warn!("Local agent not available. Bundle dropped.");
                }
            }
        } else {
            //TODO
            warn!("No local agent registered for endpoint. Bundle dropped.");
        }
    }
}
