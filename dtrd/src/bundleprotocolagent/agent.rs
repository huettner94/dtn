use std::collections::VecDeque;

use async_trait::async_trait;
use bp7::{
    administrative_record::{
        bundle_status_report::{
            BundleStatusInformation, BundleStatusItem, BundleStatusReason, BundleStatusReport,
        },
        AdministrativeRecord,
    },
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
use log::{debug, error, info, warn};
use tokio::sync::{
    mpsc::{self, error::TrySendError},
    oneshot,
};

use crate::{
    bundleprotocolagent::messages::*,
    bundlestorageagent::{self, messages::BSARequest, StoredBundle},
    clientagent::messages::{ClientAgentRequest, ListenBundlesResponse},
    common::settings::Settings,
    converganceagent::messages::{AgentForwardBundle, ConverganceAgentRequest},
    routingagent::{
        self,
        messages::{NexthopInfo, RoutingAgentRequest},
    },
};

use actix::prelude::*;

enum BundleProcessing {
    RETRY,
    FINISHED,
}

#[derive(Default)]
pub struct Daemon {
    endpoint: Option<Endpoint>,
    bsa_addr: Option<Addr<bundlestorageagent::agent::Daemon>>,
    todo_bundles: VecDeque<StoredBundle>,
}

impl Actor for Daemon {
    type Context = Context<Self>;
}
impl actix::Supervised for Daemon {}

impl SystemService for Daemon {
    fn service_started(&mut self, ctx: &mut Context<Self>) {
        let settings = Settings::from_env();
        self.endpoint = Some(Endpoint::new(&settings.my_node_id).unwrap());
        self.bsa_addr = Some(bundlestorageagent::agent::Daemon::from_registry());
    }
}

impl Handler<SendBundle> for Daemon {
    type Result = Result<(), ()>;

    fn handle(&mut self, msg: SendBundle, ctx: &mut Context<Self>) -> Self::Result {
        let SendBundle {
            destination,
            payload,
            lifetime,
        } = msg;
        let bundle = Bundle {
            primary_block: PrimaryBlock {
                version: 7,
                bundle_processing_flags: BundleFlags::BUNDLE_RECEIPTION_STATUS_REQUESTED
                    | BundleFlags::BUNDLE_FORWARDING_STATUS_REQUEST
                    | BundleFlags::BUNDLE_DELIVERY_STATUS_REQUESTED
                    | BundleFlags::BUNDLE_DELETION_STATUS_REQUESTED,
                crc: CRCType::NoCRC,
                destination_endpoint: destination,
                source_node: self.endpoint.unwrap().clone(),
                report_to: self.endpoint.unwrap().clone(),
                creation_timestamp: CreationTimestamp {
                    creation_time: DtnTime::now(),
                    sequence_number: 0, // TODO: Needs to increase for all of the same timestamp
                },
                lifetime,
                fragment_offset: None,
                total_data_length: None,
            },
            blocks: vec![CanonicalBlock {
                block: Block::Payload(PayloadBlock { data: payload }),
                block_flags: BlockFlags::empty(),
                block_number: 1,
                crc: CRCType::NoCRC,
            }],
        };
        debug!("Dispatching new bundle {:?}", &bundle.primary_block);
        self.bsa_addr
            .unwrap()
            .do_send(bundlestorageagent::messages::StoreBundle { bundle });
        let res = match bundlestorageagent::client::store_bundle(
            self.bsa_sender.as_ref().unwrap(),
            bundle,
        )
        .await
        {
            Ok(sb) => {
                self.todo_bundles.push_back(sb);
                Ok(())
            }
            Err(_) => Err(()),
        };
        res
    }
}

impl Handler<NewClientConnected> for Daemon {
    type Result = ();

    fn handle(&mut self, msg: NewClientConnected, ctx: &mut Context<Self>) {
        let NewClientConnected { destination } = msg;
        let (response_sender, response_receiver) = oneshot::channel();
        if let Err(e) =
            self.bsa_sender
                .as_ref()
                .unwrap()
                .send(BSARequest::GetBundleForDestination {
                    destination,
                    bundles: response_sender,
                })
        {
            error!("Error sending request to bsa {:?}", e);
        };

        match response_receiver.await {
            Ok(Ok(bundles)) => {
                for bundle in bundles {
                    self.todo_bundles.push_back(bundle);
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
}

impl Handler<NewRoutesAvailable> for Daemon {
    type Result = ();

    fn handle(&mut self, msg: NewRoutesAvailable, ctx: &mut Context<Self>) {
        let NewRoutesAvailable { destinations } = msg;
        for destination in destinations {
            let (response_sender, response_receiver) = oneshot::channel();
            if let Err(e) = self
                .bsa_sender
                .as_ref()
                .unwrap()
                .send(BSARequest::GetBundleForNode {
                    destination,
                    bundles: response_sender,
                })
            {
                error!("Error sending request to bsa {:?}", e);
            };

            match response_receiver.await {
                Ok(Ok(bundles)) => {
                    for bundle in bundles {
                        self.todo_bundles.push_back(bundle);
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
    }
}

impl Handler<ReceiveBundle> for Daemon {
    type Result = Result<(), ()>;

    fn handle(&mut self, msg: ReceiveBundle, ctx: &mut Context<Self>) -> Self::Result {
        let ReceiveBundle { bundle } = msg;
        debug!("Recived bundle: {:?}", bundle.primary_block);
        let res = match bundlestorageagent::client::store_bundle(
            self.bsa_sender.as_ref().unwrap(),
            bundle,
        )
        .await
        {
            Ok(sb) => {
                self.send_status_report_received(sb.get_bundle()).await;
                //TODO: Check crc or drop otherwise
                //TODO: CHeck extensions or do other stuff
                self.todo_bundles.push_back(sb);
                Ok(())
            }
            Err(_) => Err(()),
        };
        res
    }
}

impl Handler<ForwardBundleResult> for Daemon {
    type Result = ();

    fn handle(&mut self, msg: ForwardBundleResult, ctx: &mut Context<Self>) {
        let ForwardBundleResult { result, bundle } = msg;
        match result {
            Ok(_) => {
                self.send_status_report_forwarded(bundle.get_bundle()).await;
                bundlestorageagent::client::delete_bundle(self.bsa_sender.as_ref().unwrap(), bundle)
                    .await
            }
            Err(_) => {}
        }
    }
}

impl Daemon {
    async fn dispatch_bundle(&mut self, bundle: StoredBundle) {
        if bundle
            .get_bundle()
            .primary_block
            .destination_endpoint
            .matches_node(&self.endpoint.unwrap())
        {
            if bundle.get_bundle().primary_block.fragment_offset.is_some() {
                match bundlestorageagent::client::try_defragment_bundle(
                    self.bsa_sender.as_ref().unwrap(),
                    bundle,
                )
                .await
                {
                    Ok(defragmented) => match defragmented {
                        Some(defragment) => {
                            debug!("Successfully defragmented bundle");
                            self.todo_bundles.push_back(defragment);
                        }
                        None => {
                            debug!("Not yet enough fragments for defragmentation.")
                        }
                    },
                    Err(_) => {
                        info!("Some issue appeared during defragmentation.");
                    }
                }
            } else {
                match self.local_delivery(&bundle).await {
                    Ok(_) => {
                        bundlestorageagent::client::delete_bundle(
                            self.bsa_sender.as_ref().unwrap(),
                            bundle,
                        )
                        .await
                    }
                    Err(_) => {
                        info!("Some issue appeared during local delivery.");
                    }
                };
            }
        } else {
            info!(
                "Bundle is not for me, but for {}. trying to forward",
                bundle.get_bundle().primary_block.destination_endpoint
            );
            match self.forward_bundle(&bundle).await {
                BundleProcessing::FINISHED => {}
                BundleProcessing::RETRY => {
                    self.todo_bundles.push_back(bundle);
                }
            }
        }
    }

    async fn forward_bundle(&mut self, bundle: &StoredBundle) -> BundleProcessing {
        debug!("forwarding bundle {:?}", bundle.get_bundle().primary_block);

        match routingagent::client::get_next_hop(
            self.routing_agent_sender.as_ref().unwrap(),
            bundle
                .get_bundle()
                .primary_block
                .destination_endpoint
                .clone(),
        )
        .await
        {
            Some(NexthopInfo { next_hop, max_size }) => {
                debug!(
                    "Forwarding bundle destined for {} to {} with max bundle size of {:?}",
                    bundle.get_bundle().primary_block.destination_endpoint,
                    next_hop,
                    max_size
                );
                if max_size.is_some() && max_size.unwrap() < bundle.get_bundle_size() {
                    debug!("Fragmenting bundle to size {}", max_size.unwrap());
                    match bundle.get_bundle().clone().fragment(max_size.unwrap()) {
                        Ok(fragments) => {
                            debug!("Fragmented bundle into {} fragments", fragments.len());
                            for fragment in fragments {
                                let res = match bundlestorageagent::client::store_bundle(
                                    self.bsa_sender.as_ref().unwrap(),
                                    fragment,
                                )
                                .await
                                {
                                    Ok(sb) => {
                                        self.todo_bundles.push_back(sb);
                                        Ok(())
                                    }
                                    Err(_) => Err(()),
                                };
                                if res.is_err() {
                                    return BundleProcessing::RETRY;
                                }
                            }
                            return BundleProcessing::FINISHED;
                        }
                        Err(e) => {
                            error!(
                                "Error fragmenting bundle to size {}: {:?}",
                                max_size.unwrap(),
                                e
                            );
                            return BundleProcessing::RETRY;
                        }
                    }
                }
                if let Some(sender) = self.get_connected_node(next_hop).await {
                    let (send_result_sender, send_result_receiver) = oneshot::channel();
                    let result = sender.try_send(AgentForwardBundle {
                        bundle: bundle.clone(),
                        responder: send_result_sender,
                    });
                    match result {
                        Ok(_) => {
                            // since this might take arbitrary long we do not want to block
                            let sender = self.channel_sender.as_ref().unwrap().clone();
                            let cloned_bundle = bundle.clone();
                            tokio::task::spawn(async move {
                                match send_result_receiver.await {
                                    Ok(result) => match result {
                                        Ok(_) => {
                                            debug!("Bundle forwarded to remote node");
                                            if let Err(e) =
                                                sender.send(BPARequest::ForwardBundleResult {
                                                    result: Ok(()),
                                                    bundle: cloned_bundle,
                                                })
                                            {
                                                error!(
                                                    "Error sending forward result to bpa {:?}",
                                                    e
                                                );
                                            };
                                            return;
                                        }
                                        Err(_) => {
                                            warn!("Error during bundle forwarding");
                                        }
                                    },
                                    Err(_) => {
                                        warn!("Error receiving sending result");
                                    }
                                }
                                if let Err(e) = sender.send(BPARequest::ForwardBundleResult {
                                    result: Err(()),
                                    bundle: cloned_bundle,
                                }) {
                                    error!("Error sending forward result to bpa {:?}", e);
                                };
                            });
                            return BundleProcessing::FINISHED;
                        }
                        Err(TrySendError::Full(_)) => {
                            warn!("Queue to Convergance Layer Agent is full. Trying again later");
                            return BundleProcessing::RETRY;
                        }
                        Err(_) => {
                            info!("Remote node not available. Bundle queued.");
                        }
                    }
                } else {
                    info!("No remote node registered for endpoint. Bundle queued.");
                    return BundleProcessing::FINISHED;
                }
            }
            None => {
                info!("No next hop found. Bundle queued.");
                return BundleProcessing::FINISHED;
            }
        }
        return BundleProcessing::RETRY;
    }

    async fn local_delivery(&mut self, bundle: &StoredBundle) -> Result<(), ()> {
        debug!(
            "locally delivering bundle {:?}",
            &bundle.get_bundle().primary_block
        );
        if bundle.get_bundle().primary_block.fragment_offset.is_some() {
            panic!("Bundle is a fragment. It should have been reassembled before calling this");
        }

        if let Some(sender) = self
            .get_connected_client(
                bundle
                    .get_bundle()
                    .primary_block
                    .destination_endpoint
                    .clone(),
            )
            .await
        {
            let result = sender
                .send(ListenBundlesResponse {
                    data: bundle.get_bundle().payload_block().data.clone(),
                    endpoint: bundle.get_bundle().primary_block.source_node.clone(),
                })
                .await;
            match result {
                Ok(_) => {
                    debug!("Bundle dispatched to local agent");
                    self.send_status_report_delivered(bundle.get_bundle()).await;
                    Ok(())
                }
                Err(_) => {
                    info!("Local agent not available. Bundle queued.");
                    Err(())
                }
            }
        } else {
            info!(
                "No local agent registered for endpoint {}. Bundle queued.",
                bundle.get_bundle().primary_block.destination_endpoint
            );
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
        if let Err(e) = client_agent.send(ClientAgentRequest::AgentGetClient {
            destination: endpoint,
            responder: responder_sender,
        }) {
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
        if let Err(e) = convergance_agent.send(ConverganceAgentRequest::AgentGetNode {
            destination: endpoint,
            responder: responder_sender,
        }) {
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

    async fn send_status_report_received(&mut self, bundle: &Bundle) {
        if !bundle
            .primary_block
            .bundle_processing_flags
            .contains(BundleFlags::BUNDLE_RECEIPTION_STATUS_REQUESTED)
        {
            return;
        }
        self.send_status_report(
            bundle,
            BundleStatusReason::NoAdditionalInformation,
            true,
            false,
            false,
            false,
        )
        .await;
    }

    async fn send_status_report_forwarded(&mut self, bundle: &Bundle) {
        if !bundle
            .primary_block
            .bundle_processing_flags
            .contains(BundleFlags::BUNDLE_FORWARDING_STATUS_REQUEST)
        {
            return;
        }
        self.send_status_report(
            bundle,
            BundleStatusReason::NoAdditionalInformation,
            false,
            true,
            false,
            false,
        )
        .await;
    }

    async fn send_status_report_delivered(&mut self, bundle: &Bundle) {
        if !bundle
            .primary_block
            .bundle_processing_flags
            .contains(BundleFlags::BUNDLE_DELIVERY_STATUS_REQUESTED)
        {
            return;
        }
        self.send_status_report(
            bundle,
            BundleStatusReason::NoAdditionalInformation,
            false,
            false,
            true,
            false,
        )
        .await;
    }

    async fn send_status_report(
        &mut self,
        bundle: &Bundle,
        reason: BundleStatusReason,
        is_received: bool,
        is_forwarded: bool,
        is_delivered: bool,
        is_deleted: bool,
    ) {
        let now = DtnTime::now();
        let received_info = BundleStatusItem {
            is_asserted: is_received,
            timestamp: if is_received { Some(now) } else { None },
        };
        let forwarded_info = BundleStatusItem {
            is_asserted: is_forwarded,
            timestamp: if is_forwarded { Some(now) } else { None },
        };
        let delivered_info = BundleStatusItem {
            is_asserted: is_delivered,
            timestamp: if is_delivered { Some(now) } else { None },
        };
        let deleted_info = BundleStatusItem {
            is_asserted: is_deleted,
            timestamp: if is_deleted { Some(now) } else { None },
        };
        match AdministrativeRecord::BundleStatusReport(BundleStatusReport {
            status_information: BundleStatusInformation {
                received_bundle: received_info,
                forwarded_bundle: forwarded_info,
                delivered_bundle: delivered_info,
                deleted_bundle: deleted_info,
            },
            reason,
            bundle_source: bundle.primary_block.source_node.clone(),
            bundle_creation_timestamp: bundle.primary_block.creation_timestamp.clone(),
            fragment_offset: bundle.primary_block.fragment_offset,
            fragment_length: bundle.primary_block.total_data_length,
        })
        .try_into()
        {
            Ok(data) => {
                let bundle = Bundle {
                    primary_block: PrimaryBlock {
                        version: 7,
                        bundle_processing_flags: BundleFlags::ADMINISTRATIVE_RECORD,
                        crc: CRCType::NoCRC,
                        destination_endpoint: bundle.primary_block.report_to.clone(),
                        source_node: self.endpoint.unwrap().clone(),
                        report_to: self.endpoint.unwrap().clone(),
                        creation_timestamp: CreationTimestamp {
                            creation_time: DtnTime::now(),
                            sequence_number: 0, // TODO: Needs to increase for all of the same timestamp
                        },
                        lifetime: bundle.primary_block.lifetime,
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
                debug!(
                    "Dispatching administrative record bundle {:?}",
                    &bundle.primary_block
                );
                match bundlestorageagent::client::store_bundle(
                    self.bsa_sender.as_ref().unwrap(),
                    bundle,
                )
                .await
                {
                    Ok(sb) => {
                        self.todo_bundles.push_back(sb);
                    }
                    Err(_) => warn!("Could not store bundle status report for dispatching"),
                };
            }
            Err(e) => {
                warn!("Error serializing bundle status report: {:?}", e)
            }
        };
    }
}
