use tcpcl::errors::ErrorType;
use tokio::io::AsyncReadExt;

use crate::common::*;

mod common;

#[tokio::test]
async fn test_closes_on_missed_keepalive() -> Result<(), ErrorType> {
    let (jh, mut session) = setup_conn_custom_sessinit(
        |mut client| async move {
            let mut buf: [u8; 100] = [0; 100];
            let len = client.read(&mut buf).await.unwrap();
            assert_eq!(len, 1);
            assert_eq!(
                buf[0..1],
                [
                    0x04, // message type
                ]
            );

            let mut buf: [u8; 100] = [0; 100];
            let len = client.read(&mut buf).await.unwrap();
            assert_eq!(len, 1);
            assert_eq!(
                buf[0..1],
                [
                    0x04, // message type
                ]
            );

            let mut buf: [u8; 100] = [0; 100];
            let len = client.read(&mut buf).await.unwrap();
            assert_eq!(len, 4);
            assert_eq!(
                buf[0..4],
                [
                    0x04, // message type
                    0x05, // message type (term)
                    0x00, // flags
                    0x01, // idle timeout
                ]
            );
        },
        SESS_INIT_CLIENT_KEEPALIVE_1S,
    )
    .await?;

    session.manage_connection().await.unwrap();
    jh.await.unwrap();

    Ok(())
}
