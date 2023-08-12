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

use std::{future::Future, net::SocketAddrV4, str::FromStr};

use tcpcl::{errors::ErrorType, session::TCPCLSession};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    task::JoinHandle,
};

pub mod tls;

#[allow(dead_code)]
pub const CONTACT_HEADER_NO_TLS: [u8; 6] = [
    0x64, 0x74, 0x6E, 0x21, // magic "dtn!"
    0x04, // version 4
    0x00, // flags
];

#[allow(dead_code)]
pub const CONTACT_HEADER_TLS: [u8; 6] = [
    0x64, 0x74, 0x6E, 0x21, // magic "dtn!"
    0x04, // version 4
    0x01, // flags
];

#[allow(dead_code)]
pub const SESS_INIT_CLIENT: [u8; 37] = [
    0x07, // message type
    0x00, 0x3C, // keepalive_interval
    0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0x90, 0x00, // segment_mru
    0x00, 0x00, 0x00, 0x00, 0x00, 0x10, 0x00, 0x00, // transfer_mru,
    0x00, 0x0C, // node_id_len,
    0x64, 0x74, 0x6E, 0x3A, 0x2F, 0x2F, 0x63, 0x6C, 0x69, 0x65, 0x6E,
    0x74, // node_id "dtn://client"
    0x00, 0x00, 0x00, 0x00, // session extension length
];

#[allow(dead_code)]
pub const SESS_INIT_CLIENT_NAME_2: [u8; 38] = [
    0x07, // message type
    0x00, 0x3C, // keepalive_interval
    0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0x90, 0x00, // segment_mru
    0x00, 0x00, 0x00, 0x00, 0x00, 0x10, 0x00, 0x00, // transfer_mru,
    0x00, 0x0D, // node_id_len,
    0x64, 0x74, 0x6E, 0x3A, 0x2F, 0x2F, 0x63, 0x6C, 0x69, 0x65, 0x6E, 0x74,
    0x32, // node_id "dtn://client2"
    0x00, 0x00, 0x00, 0x00, // session extension length
];

#[allow(dead_code)]
pub const SESS_INIT_CLIENT_KEEPALIVE_NONE: [u8; 37] = [
    0x07, // message type
    0x00, 0x00, // keepalive_interval
    0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0x90, 0x00, // segment_mru
    0x00, 0x00, 0x00, 0x00, 0x00, 0x10, 0x00, 0x00, // transfer_mru,
    0x00, 0x0C, // node_id_len,
    0x64, 0x74, 0x6E, 0x3A, 0x2F, 0x2F, 0x63, 0x6C, 0x69, 0x65, 0x6E,
    0x74, // node_id "dtn://client"
    0x00, 0x00, 0x00, 0x00, // session extension length
];

#[allow(dead_code)]
pub const SESS_INIT_CLIENT_KEEPALIVE_1S: [u8; 37] = [
    0x07, // message type
    0x00, 0x01, // keepalive_interval
    0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0x90, 0x00, // segment_mru
    0x00, 0x00, 0x00, 0x00, 0x00, 0x10, 0x00, 0x00, // transfer_mru,
    0x00, 0x0C, // node_id_len,
    0x64, 0x74, 0x6E, 0x3A, 0x2F, 0x2F, 0x63, 0x6C, 0x69, 0x65, 0x6E,
    0x74, // node_id "dtn://client"
    0x00, 0x00, 0x00, 0x00, // session extension length
];

#[allow(dead_code)]
pub const SESS_INIT_CLIENT_SMRU_2: [u8; 37] = [
    0x07, // message type
    0x00, 0x00, // keepalive_interval
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x02, // segment_mru
    0x00, 0x00, 0x00, 0x00, 0x00, 0x10, 0x00, 0x00, // transfer_mru,
    0x00, 0x0C, // node_id_len,
    0x64, 0x74, 0x6E, 0x3A, 0x2F, 0x2F, 0x63, 0x6C, 0x69, 0x65, 0x6E,
    0x74, // node_id "dtn://client"
    0x00, 0x00, 0x00, 0x00, // session extension length
];

#[allow(dead_code)]
pub const SESS_INIT_SERVER: [u8; 37] = [
    0x07, // message type
    0x00, 0x3C, // keepalive_interval
    0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0x90, 0x00, // segment_mru
    0x00, 0x00, 0x00, 0x00, 0x00, 0x10, 0x00, 0x00, // transfer_mru,
    0x00, 0x0C, // node_id_len,
    0x64, 0x74, 0x6E, 0x3A, 0x2F, 0x2F, 0x73, 0x65, 0x72, 0x76, 0x65,
    0x72, // node_id "dtn://server"
    0x00, 0x00, 0x00, 0x00, // session extension length
];

#[allow(dead_code)]
pub const SESS_INIT_SERVER_NAME_2: [u8; 38] = [
    0x07, // message type
    0x00, 0x3C, // keepalive_interval
    0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0x90, 0x00, // segment_mru
    0x00, 0x00, 0x00, 0x00, 0x00, 0x10, 0x00, 0x00, // transfer_mru,
    0x00, 0x0D, // node_id_len,
    0x64, 0x74, 0x6E, 0x3A, 0x2F, 0x2F, 0x73, 0x65, 0x72, 0x76, 0x65, 0x72,
    0x32, // node_id "dtn://server2"
    0x00, 0x00, 0x00, 0x00, // session extension length
];

pub async fn setup_conn_custom_sessinit<Fut>(
    do_test: impl FnOnce(TcpStream) -> Fut + Send + 'static,
    sessinit: [u8; 37],
) -> Result<(JoinHandle<()>, TCPCLSession), ErrorType>
where
    Fut: Future<Output = ()> + Send,
{
    let listener = TcpListener::bind(SocketAddrV4::from_str("127.0.0.1:0").unwrap()).await?;
    let addr = listener.local_addr()?;
    let jh = tokio::spawn(async move {
        let mut client = TcpStream::connect(&addr).await.unwrap();
        client.write_all(&CONTACT_HEADER_NO_TLS).await.unwrap();

        let mut buf: [u8; 6] = [0; 6];
        client.read_exact(&mut buf).await.unwrap();

        client.write_all(&sessinit).await.unwrap();

        let mut buf: [u8; 37] = [0; 37];
        client.read_exact(&mut buf).await.unwrap();

        do_test(client).await;
    });

    let (socket, _) = listener.accept().await?;
    let session = TCPCLSession::new(socket, "dtn://server".into(), None)?;

    Ok((jh, session))
}

#[allow(dead_code)]
pub async fn setup_conn<Fut>(
    do_test: impl FnOnce(TcpStream) -> Fut + Send + 'static,
) -> Result<(JoinHandle<()>, TCPCLSession), ErrorType>
where
    Fut: Future<Output = ()> + Send,
{
    setup_conn_custom_sessinit(do_test, SESS_INIT_CLIENT_SMRU_2).await
}
