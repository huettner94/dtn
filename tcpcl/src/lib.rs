use std::{io, net::SocketAddr};

use log::{debug, info, warn};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt, Interest},
    net::{TcpListener, TcpStream},
};

use crate::messages::{
    contact_header::ContactHeader, errors::Errors, reader::Reader, statemachine::StateMachine,
    transform::Transform, Messages,
};

mod messages;

pub async fn listen(socket: SocketAddr) -> Result<(), std::io::Error> {
    let listener = TcpListener::bind(&socket).await?;
    info!("Socket open, waiting for connection");
    loop {
        let (socket, _) = listener.accept().await?;
        process_socket(socket).await?;
    }
}

pub async fn connect(socket: SocketAddr) -> Result<(), std::io::Error> {
    let mut client = TcpStream::connect(&socket).await?;
    info!("Connected to peer");
    let mut data: Vec<u8> = Vec::new();
    ContactHeader::new().write(&mut data);
    client.write(&data).await?;
    Ok(())
}

async fn process_socket(mut socket: TcpStream) -> Result<(), std::io::Error> {
    info!("New connection from {:?}", socket.peer_addr());
    let mut reader = Reader::new();
    loop {
        let ready = socket.ready(Interest::READABLE).await?;
        let sm = StateMachine::new_passive();

        if ready.is_readable() {
            match reader.read(&mut socket).await {
                Ok(0) => {
                    info!("Connection closed");
                    return Ok(());
                }
                Ok(n) => {
                    info!("read {} bytes", n);
                    let msg = sm.decode_message(&mut reader);
                    match msg {
                        Ok(Messages::ContactHeader(h)) => {
                            info!("Got contact header: {:?}", h)
                        }
                        Err(Errors::MessageTooShort) => {
                            debug!("Message was too short, retrying later");
                            continue;
                        }
                        Err(Errors::InvalidHeader) => {
                            warn!("Header invalid");
                            return Err(std::io::Error::new(
                                std::io::ErrorKind::Other,
                                "header wrong",
                            ));
                        }
                    }
                }
                Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                    continue;
                }
                Err(e) => {
                    return Err(e.into());
                }
            }
        }
    }
}
