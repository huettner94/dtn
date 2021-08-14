use dtn::bp7::{
    block::{hop_count_block::HopCountBlock, payload_block::PayloadBlock, Block, CanonicalBlock},
    blockflags::BlockFlags,
    bundle::Bundle,
    bundleflags::BundleFlags,
    crc::CRCType,
    endpoint::Endpoint,
    primaryblock::PrimaryBlock,
    time::{CreationTimestamp, DtnTime},
    SerializationError,
};

#[test]
fn test_rand_bundle_1() -> Result<(), SerializationError> {
    const BUNDLE_SRC: &str = "9F88071A00020004008201702F2F6E6F646533312F6D61766C696E6B8201702F2F6E6F6465322F696E636F6D696E678201702F2F6E6F6465322F696E636F6D696E67821B0000009E9DE3DEFE001A0036EE80850A020000448218200085010100004443414243FF";

    let recovered = Bundle::from_hex(&BUNDLE_SRC)?;

    let expected_bundle: Bundle = Bundle {
        primary_block: PrimaryBlock {
            version: 7,
            bundle_processing_flags: BundleFlags::MUST_NOT_FRAGMENT
                | BundleFlags::BUNDLE_DELIVERY_STATUS_REQUESTED,
            crc: CRCType::NoCRC,
            destination_endpoint: Endpoint::new("dtn://node31/mavlink").unwrap(),
            source_node: Endpoint::new("dtn://node2/incoming").unwrap(),
            report_to: Endpoint::new("dtn://node2/incoming").unwrap(),
            creation_timestamp: CreationTimestamp {
                creation_time: DtnTime {
                    timestamp: 681253789438,
                },
                sequence_number: 0,
            },
            lifetime: 3600000,
            fragment_offset: None,
            total_data_length: None,
        },
        blocks: [
            CanonicalBlock {
                block: Block::HopCount(HopCountBlock {
                    limit: 32,
                    count: 0,
                }),
                block_number: 2,
                block_flags: BlockFlags::empty(),
                crc: CRCType::NoCRC,
            },
            CanonicalBlock {
                block: Block::Payload(PayloadBlock {
                    data: [67, 65, 66, 67].into(),
                }),
                block_number: 1,
                block_flags: BlockFlags::empty(),
                crc: CRCType::NoCRC,
            },
        ]
        .into(),
    };

    assert_eq!(expected_bundle, recovered);

    let reserialized: String = recovered.as_hex()?;

    assert_eq!(BUNDLE_SRC, reserialized);
    Ok(())
}
