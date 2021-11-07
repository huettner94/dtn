use std::{future::Future, net::SocketAddrV4, str::FromStr, time::Duration};

use tcpcl::{errors::ErrorType, session::TCPCLSession};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    task::JoinHandle,
};

const CONTACT_HEADER_NO_TLS: [u8; 6] = [
    0x64, 0x74, 0x6E, 0x21, // magic "dtn!"
    0x04, // version 4
    0x00, // flags
];

const SESS_INIT_CLIENT: [u8; 37] = [
    0x07, // message type
    0x00, 0x00, // keepalive_interval
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x24, 0x00, // segment_mru
    0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0x86, 0xA0, // transfer_mru,
    0x00, 0x0C, // node_id_len,
    0x64, 0x74, 0x6E, 0x3A, 0x2F, 0x2F, 0x63, 0x6C, 0x69, 0x65, 0x6E,
    0x74, // node_id "dtn://client"
    0x00, 0x00, 0x00, 0x00, // session extension length
];

const SESS_INIT_CLIENT_SMRU_2: [u8; 37] = [
    0x07, // message type
    0x00, 0x00, // keepalive_interval
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x02, // segment_mru
    0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0x86, 0xA0, // transfer_mru,
    0x00, 0x0C, // node_id_len,
    0x64, 0x74, 0x6E, 0x3A, 0x2F, 0x2F, 0x63, 0x6C, 0x69, 0x65, 0x6E,
    0x74, // node_id "dtn://client"
    0x00, 0x00, 0x00, 0x00, // session extension length
];

const SESS_INIT_SERVER: [u8; 37] = [
    0x07, // message type
    0x00, 0x00, // keepalive_interval
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x24, 0x00, // segment_mru
    0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0x86, 0xA0, // transfer_mru,
    0x00, 0x0C, // node_id_len,
    0x64, 0x74, 0x6E, 0x3A, 0x2F, 0x2F, 0x73, 0x65, 0x72, 0x76, 0x65,
    0x72, // node_id "dtn://server"
    0x00, 0x00, 0x00, 0x00, // session extension length
];

async fn setup_conn<Fut>(
    do_test: impl FnOnce(TcpStream) -> Fut + Send + 'static,
) -> Result<(JoinHandle<()>, TCPCLSession), ErrorType>
where
    Fut: Future<Output = ()> + Send,
{
    let listener = TcpListener::bind(SocketAddrV4::from_str("127.0.0.1:0").unwrap()).await?;
    let addr = listener.local_addr()?;
    let jh = tokio::spawn(async move {
        let mut client = TcpStream::connect(&addr).await.unwrap();
        client.write(&CONTACT_HEADER_NO_TLS).await.unwrap();

        let mut buf: [u8; 6] = [0; 6];
        client.read(&mut buf).await.unwrap();

        client.write(&SESS_INIT_CLIENT_SMRU_2).await.unwrap();

        let mut buf: [u8; 37] = [0; 37];
        client.read(&mut buf).await.unwrap();

        do_test(client).await;
    });

    let (socket, _) = listener.accept().await?;
    let session = TCPCLSession::new(socket, "dtn://server".into())?;

    Ok((jh, session))
}

#[tokio::test]
async fn test_connection_setup_client() -> Result<(), ErrorType> {
    let listener = TcpListener::bind(SocketAddrV4::from_str("127.0.0.1:0").unwrap()).await?;
    let addr = listener.local_addr()?;
    let jh = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();

        let mut buf: [u8; 100] = [0; 100];
        let len = socket.read(&mut buf).await.unwrap();
        assert_eq!(len, 6);
        assert_eq!(buf[0..6], CONTACT_HEADER_NO_TLS);

        socket.write(&CONTACT_HEADER_NO_TLS).await.unwrap();

        let mut buf: [u8; 100] = [0; 100];
        let len = socket.read(&mut buf).await.unwrap();
        assert_eq!(len, 37);
        assert_eq!(buf[0..37], SESS_INIT_CLIENT);

        socket.write(&SESS_INIT_SERVER).await.unwrap();
    });
    let mut session = TCPCLSession::connect(addr, "dtn://client".into()).await?;
    let established = session.get_established_channel();
    session.manage_connection().await;
    jh.await.unwrap();

    let conn_info = established.await.unwrap();
    assert_eq!(conn_info.peer_endpoint.unwrap(), "dtn://server");

    Ok(())
}

#[tokio::test]
async fn test_connection_setup_server() -> Result<(), ErrorType> {
    let listener = TcpListener::bind(SocketAddrV4::from_str("127.0.0.1:0").unwrap()).await?;
    let addr = listener.local_addr()?;
    let jh = tokio::spawn(async move {
        let mut client = TcpStream::connect(&addr).await.unwrap();
        client.write(&CONTACT_HEADER_NO_TLS).await.unwrap();

        let mut buf: [u8; 100] = [0; 100];
        let len = client.read(&mut buf).await.unwrap();
        assert_eq!(len, 6);
        assert_eq!(buf[0..6], CONTACT_HEADER_NO_TLS);

        client.write(&SESS_INIT_CLIENT).await.unwrap();

        let mut buf: [u8; 100] = [0; 100];
        let len = client.read(&mut buf).await.unwrap();
        assert_eq!(len, 37);
        assert_eq!(buf[0..37], SESS_INIT_SERVER);
    });

    let (socket, _) = listener.accept().await?;
    let mut session = TCPCLSession::new(socket, "dtn://server".into())?;
    let established = session.get_established_channel();
    session.manage_connection().await;
    jh.await.unwrap();

    let conn_info = established.await.unwrap();
    assert_eq!(conn_info.peer_endpoint.unwrap(), "dtn://client");

    Ok(())
}

#[tokio::test]
async fn test_session_termination_receive() -> Result<(), ErrorType> {
    let (jh, mut session) = setup_conn(|mut client| async move {
        client
            .write(&[
                0x05, // message type
                0x00, // flags
                0x03, // reason (busy)
            ])
            .await
            .unwrap();

        let mut buf: [u8; 100] = [0; 100];
        let len = client.read(&mut buf).await.unwrap();
        assert_eq!(len, 3);
        assert_eq!(
            buf[0..3],
            [
                0x05, // message type
                0x01, // flags (reply)
                0x03, // reason (busy)
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

#[tokio::test]
async fn test_session_termination_send() -> Result<(), ErrorType> {
    let (jh, mut session) = setup_conn(|mut client| async move {
        let mut buf: [u8; 100] = [0; 100];
        let len = client.read(&mut buf).await.unwrap();
        assert_eq!(len, 3);
        assert_eq!(
            buf[0..3],
            [
                0x05, // message type
                0x00, // flags
                0x05, // reason (resource exhaustion)
            ]
        );

        client
            .write(&[
                0x05, // message type
                0x01, // flags (reply)
                0x05, // reason (resource exhaustion)
            ])
            .await
            .unwrap();

        let mut buf: [u8; 100] = [0; 100];
        let len = client.read(&mut buf).await.unwrap();
        assert_eq!(len, 0);
    })
    .await?;

    let established_channel = session.get_established_channel();
    let close_channel = session.get_close_channel();

    tokio::spawn(async move {
        established_channel.await.unwrap();
        close_channel.send(()).unwrap();
    });

    session.manage_connection().await;
    jh.await.unwrap();

    Ok(())
}

#[tokio::test]
async fn test_xfer_single_segment_receive() -> Result<(), ErrorType> {
    let (jh, mut session) = setup_conn(|mut client| async move {
        client
            .write(&[
                0x01, // message type
                0x03, // flags (start + end)
                0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, // transfer id
                0x00, 0x00, 0x00, 0x00, // transfer extensions
                0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x02, // data bytes
                0x55, 0xAA, // data
            ])
            .await
            .unwrap();

        let mut buf: [u8; 100] = [0; 100];
        let len = client.read(&mut buf).await.unwrap();
        assert_eq!(len, 18);
        assert_eq!(
            buf[0..18],
            [
                0x02, // message type
                0x03, // flags (start + end)
                0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, // transfer id
                0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x02, // ack length
            ]
        );
    })
    .await?;

    let mut receive_channel = session.get_receive_channel();

    session.manage_connection().await;
    jh.await.unwrap();

    let received = receive_channel.recv().await.unwrap();
    assert_eq!(received.id, 1);
    assert_eq!(received.data, [0x55, 0xAA]);

    Ok(())
}

#[tokio::test]
async fn test_xfer_single_multi_receive() -> Result<(), ErrorType> {
    let (jh, mut session) = setup_conn(|mut client| async move {
        client
            .write(&[
                0x01, // message type
                0x02, // flags (start)
                0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, // transfer id
                0x00, 0x00, 0x00, 0x00, // transfer extensions
                0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x02, // data bytes
                0x55, 0xAA, // data
            ])
            .await
            .unwrap();

        let mut buf: [u8; 100] = [0; 100];
        let len = client.read(&mut buf).await.unwrap();
        assert_eq!(len, 18);
        assert_eq!(
            buf[0..18],
            [
                0x02, // message type
                0x02, // flags (start)
                0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, // transfer id
                0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x02, // ack length
            ]
        );

        client
            .write(&[
                0x01, // message type
                0x01, // flags (end)
                0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, // transfer id
                0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x02, // data bytes
                0xAA, 0x55, // data
            ])
            .await
            .unwrap();

        let mut buf: [u8; 100] = [0; 100];
        let len = client.read(&mut buf).await.unwrap();
        assert_eq!(len, 18);
        assert_eq!(
            buf[0..18],
            [
                0x02, // message type
                0x01, // flags (end)
                0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, // transfer id
                0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x04, // ack length
            ]
        );
    })
    .await?;

    let mut receive_channel = session.get_receive_channel();

    session.manage_connection().await;
    jh.await.unwrap();

    let received = receive_channel.recv().await.unwrap();
    assert_eq!(received.id, 1);
    assert_eq!(received.data, [0x55, 0xAA, 0xAA, 0x55]);

    Ok(())
}

#[tokio::test]
async fn test_xfer_single_segment_send() -> Result<(), ErrorType> {
    let (jh, mut session) = setup_conn(|mut client| async move {
        let mut buf: [u8; 100] = [0; 100];
        let len = client.read(&mut buf).await.unwrap();
        assert_eq!(len, 24);
        assert_eq!(
            buf[0..24],
            [
                0x01, // message type
                0x03, // flags (start + end)
                0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // transfer id
                0x00, 0x00, 0x00, 0x00, // transfer extensions
                0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x02, // data bytes
                0x55, 0xAA, // data
            ]
        );

        client
            .write(&[
                0x02, // message type
                0x03, // flags (start + end)
                0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // transfer id
                0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x02, // ack length
            ])
            .await
            .unwrap();
    })
    .await?;

    let established_channel = session.get_established_channel();
    let send_channel = session.get_send_channel();

    tokio::spawn(async move {
        established_channel.await.unwrap();
        send_channel.send([0x55, 0xAA].into()).await.unwrap();
    });

    session.manage_connection().await;
    jh.await.unwrap();

    Ok(())
}

#[tokio::test]
async fn test_xfer_multi_segment_send() -> Result<(), ErrorType> {
    let (jh, mut session) = setup_conn(|mut client| async move {
        let mut buf: [u8; 24] = [0; 24];
        let len = client.read(&mut buf).await.unwrap();
        assert_eq!(len, 24);
        assert_eq!(
            buf[0..24],
            [
                0x01, // message type
                0x02, // flags (start)
                0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // transfer id
                0x00, 0x00, 0x00, 0x00, // transfer extensions
                0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x02, // data bytes
                0x55, 0xAA, // data
            ]
        );

        client
            .write(&[
                0x02, // message type
                0x02, // flags (start)
                0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // transfer id
                0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x02, // ack length
            ])
            .await
            .unwrap();

        let mut buf: [u8; 100] = [0; 100];
        let len = client.read(&mut buf).await.unwrap();
        assert_eq!(len, 20);
        assert_eq!(
            buf[0..20],
            [
                0x01, // message type
                0x01, // flags (end)
                0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // transfer id
                0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x02, // data bytes
                0xAA, 0x55, // data
            ]
        );

        client
            .write(&[
                0x02, // message type
                0x01, // flags (end)
                0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // transfer id
                0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x04, // ack length
            ])
            .await
            .unwrap();
    })
    .await?;

    let established_channel = session.get_established_channel();
    let send_channel = session.get_send_channel();

    tokio::spawn(async move {
        established_channel.await.unwrap();
        send_channel
            .send([0x55, 0xAA, 0xAA, 0x55].into())
            .await
            .unwrap();
    });

    session.manage_connection().await;
    jh.await.unwrap();

    Ok(())
}
