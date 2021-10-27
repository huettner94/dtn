use std::{io::ErrorKind, net::SocketAddr, time::Duration};

use errors::ErrorType;
use log::{info, warn};
use session::TCPCLSession;
use tokio::{
    net::{TcpListener, TcpStream},
    time::sleep,
};

pub mod connection_info;
pub mod errors;
pub mod session;
pub mod transfer;
mod v4;

pub async fn listen(socket: SocketAddr) -> Result<(), std::io::Error> {
    let listener = TcpListener::bind(&socket).await?;
    info!("Socket open, waiting for connection");
    loop {
        let (socket, _) = listener.accept().await?;
        match process_socket(socket).await {
            Ok(_) => {}
            Err(e) if e.kind() == ErrorKind::ConnectionReset => {
                warn!("Remote reset connection unexpectedly");
            }
            Err(e) => return Err(e),
        };
    }
}

pub async fn connect(socket: SocketAddr) -> Result<(), ErrorType> {
    let mut sess = TCPCLSession::connect(socket, "dtn://receiver".into()).await?;
    let close_channel = sess.get_close_channel();
    let transfer_send = sess.get_send_channel();
    let mut receiver = sess.get_receive_channel();
    let jh = tokio::spawn(async move { sess.manage_connection().await });

    tokio::spawn(async move {
        loop {
            match receiver.recv().await {
                Some(t) => {
                    info!("Received transfer {:?}", t)
                }
                None => break,
            }
        }
    });

    info!("Now sleeping for 1 secs");
    sleep(Duration::from_secs(1)).await;
    info!("Will now send a bundle");

    match transfer_send
        .send(
            [
                0x9F, 0x88, 0x07, 0x1A, 0x00, 0x02, 0x00, 0x04, 0x00, 0x82, 0x01, 0x70, 0x2F, 0x2F,
                0x6E, 0x6F, 0x64, 0x65, 0x33, 0x31, 0x2F, 0x6D, 0x61, 0x76, 0x6C, 0x69, 0x6E, 0x6B,
                0x82, 0x01, 0x70, 0x2F, 0x2F, 0x6E, 0x6F, 0x64, 0x65, 0x32, 0x2F, 0x69, 0x6E, 0x63,
                0x6F, 0x6D, 0x69, 0x6E, 0x67, 0x82, 0x01, 0x70, 0x2F, 0x2F, 0x6E, 0x6F, 0x64, 0x65,
                0x32, 0x2F, 0x69, 0x6E, 0x63, 0x6F, 0x6D, 0x69, 0x6E, 0x67, 0x82, 0x1B, 0x00, 0x00,
                0x00, 0x9E, 0x9D, 0xE3, 0xDE, 0xFE, 0x00, 0x1A, 0x00, 0x36, 0xEE, 0x80, 0x85, 0x0A,
                0x02, 0x00, 0x00, 0x44, 0x82, 0x18, 0x20, 0x00, 0x85, 0x01, 0x01, 0x00, 0x00, 0x44,
                0x43, 0x41, 0x42, 0x43, 0xFF,
            ]
            .into(),
        )
        .await
    {
        Ok(_) => {}
        Err(_) => {
            warn!("Some channel error happened")
        }
    };

    info!("Now sleeping for 120 secs");
    sleep(Duration::from_secs(120)).await;
    info!("Will now close the session");
    match close_channel.send(()) {
        Ok(_) => {}
        Err(_) => {
            warn!("Some channel error happened")
        }
    };

    match jh.await {
        Ok(_) => {}
        Err(_) => {
            warn!("A join error happened")
        }
    }
    Ok(())
}

async fn process_socket(socket: TcpStream) -> Result<(), std::io::Error> {
    info!("New connection from {}", socket.peer_addr()?);

    let mut sess = TCPCLSession::new(socket, "dtn://server".into())?;

    let mut receiver = sess.get_receive_channel();

    let jh = tokio::spawn(async move { sess.manage_connection().await });
    tokio::spawn(async move {
        loop {
            match receiver.recv().await {
                Some(t) => {
                    info!("Received transfer {:?}", t)
                }
                None => break,
            }
        }
    });

    match jh.await {
        Ok(_) => {}
        Err(_) => {
            warn!("A join error happened")
        }
    };

    Ok(())
}
