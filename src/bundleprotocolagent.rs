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
use log::info;

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

struct Daemon {
    todo: Vec<BundleProcessing>,
    endpoint: Endpoint,
}

impl Daemon {
    pub fn new() -> Self {
        Daemon {
            todo: Vec::new(),
            endpoint: Endpoint::new(&"dtn://itsme").unwrap(),
        }
    }

    fn transmit_bundle(&mut self, destination: Endpoint, data: Vec<u8>, lifetime: u64) {
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
                        sequence_number: 0, // Needs to increase for all of the same timestamp
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
        info!("Adding new bundle to todo list {:?}", &bundle);
        self.todo.push(bundle);
    }

    fn dispatch_bundle(&mut self, bundle: BundleProcessing) {
        if bundle.bundle.primary_block.destination_endpoint == self.endpoint {
            self.local_delivery(bundle);
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

    fn receive_bundle(&mut self, mut bundle: BundleProcessing) {
        bundle.bundle_constraint = Some(BundleConstraint::DispatchPending);
        //TODO: send status report if reqeusted
        //TODO: Check crc or drop otherwise
        //TODO: CHeck extensions or do other stuff
        self.dispatch_bundle(bundle);
    }

    fn local_delivery(&mut self, bundle: BundleProcessing) {
        info!("Now locally delivering bundle {:?}", &bundle);
        if bundle.bundle.primary_block.fragment_offset.is_some() {
            info!("Bundle if a fragment. No idea what to do");
            return;
        }
        //TODO: do real local delivery
        //TODO: send status report if reqeusted
    }
}
