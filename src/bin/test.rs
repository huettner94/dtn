use chrono::Utc;
use dtn::{
    block::Block, blockflags::BlockFlags, bundle::Bundle, bundleflags::BundleFlags, crc::CRCType,
    endpoint::Endpoint, primaryblock::PrimaryBlock, time::*, *,
};

static TEST: [u8; 84] = [
    0x9f, 0x89, 0x07, 0x00, 0x01, 0x82, 0x01, 0x6b, 0x6e, 0x6f, 0x64, 0x65, 0x33, 0x2f, 0x69, 0x6e,
    0x62, 0x6f, 0x78, 0x82, 0x01, 0x6b, 0x6e, 0x6f, 0x64, 0x65, 0x33, 0x2f, 0x69, 0x6e, 0x62, 0x6f,
    0x78, 0x82, 0x01, 0x6b, 0x6e, 0x6f, 0x64, 0x65, 0x33, 0x2f, 0x69, 0x6e, 0x62, 0x6f, 0x78, 0x82,
    0x1a, 0x24, 0x79, 0x66, 0xba, 0x00, 0x1a, 0xd6, 0x93, 0xa4, 0x00, 0x42, 0x25, 0xb6, 0x86, 0x01,
    0x00, 0x00, 0x01, 0x43, 0x41, 0x42, 0x43, 0x42, 0x23, 0x71, 0x86, 0x08, 0x01, 0x00, 0x01, 0x00,
    0x42, 0xdb, 0xcc, 0xff,
];

fn main() {
    /*let b = Bundle {
        primary_block: PrimaryBlock {
            version: 7,
            bundle_processing_flags: BundleFlags::MUST_NOT_FRAGMENT
                | BundleFlags::BUNDLE_RECEIPTION_STATUS_REQUESTED,
            crc: CRCType::CRC16([1, 2]),
            creation_timestamp: CreationTimestamp {
                creation_time: Utc::now().into(),
                sequence_number: 1,
            },
            destination_endpoint: Endpoint::new("dtn://destnode").unwrap(),
            source_node: Endpoint::new("dtn://none").unwrap(),
            report_to: Endpoint::new("dtn://srcnode").unwrap(),
            lifetime: 123,
            fragment_offset: None,
            total_data_length: None,
        },
        blocks: [Block {
            block_flags: BlockFlags::DELETE_BUNDLE_WHEN_NOT_PROCESSABLE,
            block_number: 1,
            block_type: block::BlockType::PAYLOAD,
            crc: CRCType::NoCRC,
            data: [].into(),
        }]
        .into(),
    };
    let v = serde_cbor::to_vec(&b).unwrap();
    println!("{:x?}; {}", v, b.validate());

    let recovered: Bundle = serde_cbor::from_slice(&v).unwrap();
    println!("{:?}; {}", &recovered, recovered.validate());*/

    let recovered: Bundle = serde_cbor::from_slice(&TEST).unwrap();
    println!("{:?}; {}", &recovered, recovered.validate());
}
