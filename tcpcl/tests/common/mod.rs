use std::{future::Future, net::SocketAddrV4, str::FromStr};

use tcpcl::{errors::ErrorType, session::TCPCLSession};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    task::JoinHandle,
};

pub const CONTACT_HEADER_NO_TLS: [u8; 6] = [
    0x64, 0x74, 0x6E, 0x21, // magic "dtn!"
    0x04, // version 4
    0x00, // flags
];

#[allow(dead_code)]
pub const SESS_INIT_CLIENT: [u8; 37] = [
    0x07, // message type
    0x00, 0x3C, // keepalive_interval
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x24, 0x00, // segment_mru
    0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0x86, 0xA0, // transfer_mru,
    0x00, 0x0C, // node_id_len,
    0x64, 0x74, 0x6E, 0x3A, 0x2F, 0x2F, 0x63, 0x6C, 0x69, 0x65, 0x6E,
    0x74, // node_id "dtn://client"
    0x00, 0x00, 0x00, 0x00, // session extension length
];

#[allow(dead_code)]
pub const SESS_INIT_CLIENT_KEEPALIVE_1S: [u8; 37] = [
    0x07, // message type
    0x00, 0x01, // keepalive_interval
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x24, 0x00, // segment_mru
    0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0x86, 0xA0, // transfer_mru,
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
    0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0x86, 0xA0, // transfer_mru,
    0x00, 0x0C, // node_id_len,
    0x64, 0x74, 0x6E, 0x3A, 0x2F, 0x2F, 0x63, 0x6C, 0x69, 0x65, 0x6E,
    0x74, // node_id "dtn://client"
    0x00, 0x00, 0x00, 0x00, // session extension length
];

#[allow(dead_code)]
pub const SESS_INIT_SERVER: [u8; 37] = [
    0x07, // message type
    0x00, 0x3C, // keepalive_interval
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x24, 0x00, // segment_mru
    0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0x86, 0xA0, // transfer_mru,
    0x00, 0x0C, // node_id_len,
    0x64, 0x74, 0x6E, 0x3A, 0x2F, 0x2F, 0x73, 0x65, 0x72, 0x76, 0x65,
    0x72, // node_id "dtn://server"
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
    let session = TCPCLSession::new(socket, "dtn://server".into())?;

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
