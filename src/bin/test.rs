use binascii::hex2bin;
use chrono::Utc;
use dtn::{
    block::Block, blockflags::BlockFlags, bundle::Bundle, bundleflags::BundleFlags, crc::CRCType,
    endpoint::Endpoint, primaryblock::PrimaryBlock, time::*, *,
};

const  TEST: &str = "9f88071a00020004008201702f2f6e6f646533312f6d61766c696e6b8201702f2f6e6f6465322f696e636f6d696e678201702f2f6e6f6465322f696e636f6d696e67821b0000009e9de3defe001a0036ee80850a020000448218200085010100004443414243ff";

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

    let mut val = [0; TEST.len() / 2];
    hex2bin(TEST.as_bytes(), &mut val).unwrap();

    let recovered: Bundle = serde_cbor::from_slice(&val).unwrap();
    println!("{:?}; {}", &recovered, recovered.validate());
}
