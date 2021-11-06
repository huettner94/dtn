use std::{net::SocketAddrV4, str::FromStr, time::Duration};

use tcpcl::{errors::ErrorType, session::TCPCLSession};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
};

#[tokio::test]
async fn test_connection_setup_client() -> Result<(), ErrorType> {
    let listener = TcpListener::bind(SocketAddrV4::from_str("127.0.0.1:0").unwrap()).await?;
    let addr = listener.local_addr()?;
    let jh = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();

        let mut buf: [u8; 100] = [0; 100];
        let len = socket.read(&mut buf).await.unwrap();
        assert_eq!(len, 6);
        assert_eq!(buf[0..6], [0x64, 0x74, 0x6E, 0x21, 4, 0x00]);

        socket
            .write(&[0x64, 0x74, 0x6E, 0x21, 4, 0x00])
            .await
            .unwrap();

        let mut buf: [u8; 100] = [0; 100];
        let len = socket.read(&mut buf).await.unwrap();
        assert_eq!(len, 37);
        assert_eq!(
            buf[0..37],
            [
                0x07, // message type
                0x00, 0x00, // keepalive_interval
                0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x24, 0x00, // segment_mru
                0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0x86, 0xA0, // transfer_mru,
                0x00, 0x0C, // node_id_len,
                0x64, 0x74, 0x6E, 0x3A, 0x2F, 0x2F, 0x63, 0x6C, 0x69, 0x65, 0x6E,
                0x74, // node_id "dtn://client"
                0x00, 0x00, 0x00, 0x00 // session extension length
            ]
        );

        socket
            .write(&[
                0x07, // message type
                0x00, 0x00, // keepalive_interval
                0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x24, 0x00, // segment_mru
                0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0x86, 0xA0, // transfer_mru,
                0x00, 0x0C, // node_id_len,
                0x64, 0x74, 0x6E, 0x3A, 0x2F, 0x2F, 0x73, 0x65, 0x72, 0x76, 0x65,
                0x72, // node_id "dtn://server"
                0x00, 0x00, 0x00, 0x00, // session extension length
            ])
            .await
            .unwrap();
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
        client
            .write(&[0x64, 0x74, 0x6E, 0x21, 4, 0x00])
            .await
            .unwrap();

        let mut buf: [u8; 100] = [0; 100];
        let len = client.read(&mut buf).await.unwrap();
        assert_eq!(len, 6);
        assert_eq!(buf[0..6], [0x64, 0x74, 0x6E, 0x21, 4, 0x00]);

        client
            .write(&[
                0x07, // message type
                0x00, 0x00, // keepalive_interval
                0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x24, 0x00, // segment_mru
                0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0x86, 0xA0, // transfer_mru,
                0x00, 0x0C, // node_id_len,
                0x64, 0x74, 0x6E, 0x3A, 0x2F, 0x2F, 0x63, 0x6C, 0x69, 0x65, 0x6E,
                0x74, // node_id "dtn://client"
                0x00, 0x00, 0x00, 0x00, // session extension length
            ])
            .await
            .unwrap();

        let mut buf: [u8; 100] = [0; 100];
        let len = client.read(&mut buf).await.unwrap();
        assert_eq!(len, 37);
        assert_eq!(
            buf[0..37],
            [
                0x07, // message type
                0x00, 0x00, // keepalive_interval
                0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x24, 0x00, // segment_mru
                0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0x86, 0xA0, // transfer_mru,
                0x00, 0x0C, // node_id_len,
                0x64, 0x74, 0x6E, 0x3A, 0x2F, 0x2F, 0x73, 0x65, 0x72, 0x76, 0x65,
                0x72, // node_id "dtn://server"
                0x00, 0x00, 0x00, 0x00 // session extension length
            ]
        );
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
async fn test_session_termination_received() -> Result<(), ErrorType> {
    let listener = TcpListener::bind(SocketAddrV4::from_str("127.0.0.1:0").unwrap()).await?;
    let addr = listener.local_addr()?;
    let jh = tokio::spawn(async move {
        let mut client = TcpStream::connect(&addr).await.unwrap();
        client
            .write(&[0x64, 0x74, 0x6E, 0x21, 4, 0x00])
            .await
            .unwrap();

        let mut buf: [u8; 100] = [0; 100];
        client.read(&mut buf).await.unwrap();

        client
            .write(&[
                0x07, // message type
                0x00, 0x00, // keepalive_interval
                0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x24, 0x00, // segment_mru
                0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0x86, 0xA0, // transfer_mru,
                0x00, 0x0C, // node_id_len,
                0x64, 0x74, 0x6E, 0x3A, 0x2F, 0x2F, 0x63, 0x6C, 0x69, 0x65, 0x6E,
                0x74, // node_id "dtn://client"
                0x00, 0x00, 0x00, 0x00, // session extension length
            ])
            .await
            .unwrap();

        let mut buf: [u8; 100] = [0; 100];
        client.read(&mut buf).await.unwrap();

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
    });

    let (socket, _) = listener.accept().await?;
    let mut session = TCPCLSession::new(socket, "dtn://server".into())?;
    session.manage_connection().await;
    jh.await.unwrap();

    Ok(())
}

#[tokio::test]
async fn test_session_termination_send() -> Result<(), ErrorType> {
    let listener = TcpListener::bind(SocketAddrV4::from_str("127.0.0.1:0").unwrap()).await?;
    let addr = listener.local_addr()?;
    let jh = tokio::spawn(async move {
        let mut client = TcpStream::connect(&addr).await.unwrap();
        client
            .write(&[0x64, 0x74, 0x6E, 0x21, 4, 0x00])
            .await
            .unwrap();

        let mut buf: [u8; 100] = [0; 100];
        client.read(&mut buf).await.unwrap();

        client
            .write(&[
                0x07, // message type
                0x00, 0x00, // keepalive_interval
                0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x24, 0x00, // segment_mru
                0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0x86, 0xA0, // transfer_mru,
                0x00, 0x0C, // node_id_len,
                0x64, 0x74, 0x6E, 0x3A, 0x2F, 0x2F, 0x63, 0x6C, 0x69, 0x65, 0x6E,
                0x74, // node_id "dtn://client"
                0x00, 0x00, 0x00, 0x00, // session extension length
            ])
            .await
            .unwrap();

        let mut buf: [u8; 100] = [0; 100];
        client.read(&mut buf).await.unwrap();

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
    });

    let (socket, _) = listener.accept().await?;
    let mut session = TCPCLSession::new(socket, "dtn://server".into())?;
    let close_channel = session.get_close_channel();

    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_secs(1)).await;
        close_channel.send(()).unwrap();
    });

    session.manage_connection().await;
    jh.await.unwrap();

    Ok(())
}