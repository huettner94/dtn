use dtn::{
    bundle::Bundle, bundleflags::BundleFlags, crc::CRCType, endpoint::Endpoint,
    primaryblock::PrimaryBlock, *,
};

fn main() {
    let b = Bundle {
        primary_block: PrimaryBlock {
            version: 7,
            bundle_processing_flags: BundleFlags::MUST_NOT_FRAGMENT
                | BundleFlags::BUNDLE_RECEIPTION_STATUS_REQUESTED,
            crc: CRCType::CRC16([1, 2]),
            creation_timestamp: CreationTimestamp {
                creation_time: 123,
                sequence_number: 1,
            },
            destination_endpoint: Endpoint::new("dtn://destnode").unwrap(),
            source_node: Endpoint::new("dtn://none").unwrap(),
            report_to: Endpoint::new("dtn://srcnode").unwrap(),
            lifetime: 123,
            fragment_offset: None,
            total_data_length: None,
        },
    };
    let v = serde_cbor::to_vec(&b).unwrap();
    println!("{:x?}; {}", v, b.validate());

    let recovered: Bundle = serde_cbor::from_slice(&v).unwrap();
    println!("{:?}; {}", &recovered, recovered.validate());
}
