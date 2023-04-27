use tcpcl::errors::{ErrorType, Errors};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

use crate::common::*;

mod common;

#[tokio::test]
async fn test_unkown_message_type() -> Result<(), ErrorType> {
    let (jh, mut session) = setup_conn(|mut client| async move {
        client
            .write(&[
                0x01, // message type
                0x02, // flags (start)
                0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // transfer id
                0x00, 0x00, 0x00, 0x00, // transfer extensions
                0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, // data bytes
                0x55, 0xAA, // fake data
            ])
            .await
            .unwrap();

        let mut buf: [u8; 100] = [0; 100];
        let len = client.read(&mut buf).await.unwrap();
        assert_eq!(len, 0);
    })
    .await?;

    let ret = session.manage_connection().await;
    if let Err(ErrorType::TCPCLError(Errors::MessageError(
        tcpcl::v4::messages::Errors::SegmentTooLong,
    ))) = ret
    {
        assert!(true);
    } else {
        assert!(false);
    }
    jh.await.unwrap();

    Ok(())
}
