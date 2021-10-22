use std::{io::ErrorKind, net::SocketAddr, time::Duration};

use errors::ErrorType;
use log::{info, warn};
use session::TCPCLSession;
use tokio::{
    net::{TcpListener, TcpStream},
    time::sleep,
};

use crate::transfer::Transfer;

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
    let mut sess = TCPCLSession::connect(socket).await?;
    let close_channel = sess.get_close_channel();
    let transfer_send = sess.get_send_channel();
    let jh = tokio::spawn(async move { sess.manage_connection().await });

    info!("Now sleeping for 1 secs");
    sleep(Duration::from_secs(1)).await;
    info!("Will now send a bundle");

    match transfer_send
        .send(Transfer {
            id: 12345,
            data: "this is a test. Lets see what happens :)".as_bytes().into(),
        })
        .await
    {
        Ok(_) => {}
        Err(_) => {
            warn!("Some channel error happened")
        }
    };

    info!("Now sleeping for 5 secs");
    sleep(Duration::from_secs(5)).await;
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

    let mut sess = TCPCLSession::new(socket);

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
