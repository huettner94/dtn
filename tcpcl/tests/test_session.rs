// Copyright (C) 2023 Felix Huettner
//
// This file is part of DTRD.
//
// DTRD is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// DTRD is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with this program.  If not, see <https://www.gnu.org/licenses/>.

use std::{net::SocketAddrV4, str::FromStr, sync::Arc};

use tcpcl::{
    errors::{ErrorType, Errors},
    session::TCPCLSession,
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    sync::oneshot,
};
use url::Url;

use crate::common::*;

mod common;

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
    let url = Url::parse(&format!("tcpcl://{}", addr)).unwrap();
    let mut session = TCPCLSession::connect(url, "dtn://client".into(), None).await?;
    let established = session.get_established_channel();
    session.manage_connection().await.unwrap();
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

        client
            .write(&SESS_INIT_CLIENT_KEEPALIVE_NONE)
            .await
            .unwrap();

        let mut buf: [u8; 100] = [0; 100];
        let len = client.read(&mut buf).await.unwrap();
        assert_eq!(len, 37);
        assert_eq!(buf[0..37], SESS_INIT_SERVER);
    });

    let (socket, _) = listener.accept().await?;
    let mut session = TCPCLSession::new(socket, "dtn://server".into(), None)?;
    let established = session.get_established_channel();
    session.manage_connection().await.unwrap();
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

    session.manage_connection().await.unwrap();
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

    session.manage_connection().await.unwrap();
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

    session.manage_connection().await.unwrap();
    jh.await.unwrap();

    let received = receive_channel.recv().await.unwrap();
    assert_eq!(received.id, 1);
    assert_eq!(Arc::try_unwrap(received.data).unwrap(), [0x55, 0xAA]);

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

    session.manage_connection().await.unwrap();
    jh.await.unwrap();

    let received = receive_channel.recv().await.unwrap();
    assert_eq!(received.id, 1);
    assert_eq!(
        Arc::try_unwrap(received.data).unwrap(),
        [0x55, 0xAA, 0xAA, 0x55]
    );

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

    let (transfer_result_sender, transfer_result_receiver) = oneshot::channel();
    tokio::spawn(async move {
        established_channel.await.unwrap();
        send_channel
            .send((Arc::new([0x55, 0xAA].into()), transfer_result_sender))
            .await
            .unwrap();
    });

    session.manage_connection().await.unwrap();
    jh.await.unwrap();

    transfer_result_receiver.await.unwrap().unwrap();

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

    let (transfer_result_sender, transfer_result_receiver) = oneshot::channel();
    tokio::spawn(async move {
        established_channel.await.unwrap();
        send_channel
            .send((
                Arc::new([0x55, 0xAA, 0xAA, 0x55].into()),
                transfer_result_sender,
            ))
            .await
            .unwrap();
    });

    session.manage_connection().await.unwrap();
    jh.await.unwrap();

    transfer_result_receiver.await.unwrap().unwrap();

    Ok(())
}

#[tokio::test]
async fn test_sends_keepalive() -> Result<(), ErrorType> {
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

            client
                .write(&[
                    0x04, // message type
                ])
                .await
                .unwrap();

            let mut buf: [u8; 100] = [0; 100];
            let len = client.read(&mut buf).await.unwrap();
            assert_eq!(len, 1);
            assert_eq!(
                buf[0..1],
                [
                    0x04, // message type
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

#[tokio::test]
async fn test_closes_on_msg_reject() -> Result<(), ErrorType> {
    let (jh, mut session) = setup_conn(|mut client| async move {
        client
            .write(&[
                0x06, // message type
                0x01, // reason code
                0xFF, // wrong message type
            ])
            .await
            .unwrap();

        let mut buf: [u8; 100] = [0; 100];
        let len = client.read(&mut buf).await.unwrap();
        assert_eq!(len, 0);
    })
    .await?;

    let ret = session.manage_connection().await;
    if let Err(ErrorType::TCPCLError(Errors::RemoteRejected)) = ret {
        assert!(true);
    } else {
        assert!(false);
    }
    jh.await.unwrap();

    Ok(())
}
