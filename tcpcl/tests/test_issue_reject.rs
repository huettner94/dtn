use tcpcl::errors::ErrorType;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

use crate::common::*;

mod common;

#[tokio::test]
async fn test_unkown_message_type() -> Result<(), ErrorType> {
    let (jh, mut session) = setup_conn(|mut client| async move {
        client
            .write(&[
                0xFF, // non existing message type
            ])
            .await
            .unwrap();

        let mut buf: [u8; 100] = [0; 100];
        let len = client.read(&mut buf).await.unwrap();
        assert_eq!(len, 3);
        assert_eq!(
            buf[0..3],
            [
                0x06, // message type
                0x01, // reason code
                0xFF, // our message type
            ]
        );

        let mut buf: [u8; 100] = [0; 100];
        let len = client.read(&mut buf).await.unwrap();
        assert_eq!(len, 0);
    })
    .await?;

    session.manage_connection().await;
    jh.await.unwrap();

    Ok(())
}
