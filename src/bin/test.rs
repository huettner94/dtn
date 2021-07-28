use std::str::FromStr;

use dtn::{block::PrimaryBlock, bundle::Bundle, crc::CRCType, *};

fn main() {
    let b = Bundle {
        primary_block: PrimaryBlock {
            version: 7,
            bundle_processing_flags: 0,
            crc: CRCType::CRC16([1, 2]),
            creation_timestamp: CreationTimestamp {
                creation_time: 123,
                sequence_number: 1,
            },
            destination_endpoint: EndpointID {
                endpoint_type: EndpointType::DTN,
                endpoint: String::from_str("abc").unwrap(),
            },
            source_node: EndpointID {
                endpoint_type: EndpointType::DTN,
                endpoint: String::from_str("def").unwrap(),
            },
            report_to: EndpointID {
                endpoint_type: EndpointType::DTN,
                endpoint: String::from_str("xcb").unwrap(),
            },
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
