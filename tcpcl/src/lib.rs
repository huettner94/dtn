use std::{io, net::SocketAddr};

use log::{debug, info, warn};
use tokio::{
    io::AsyncWriteExt,
    net::{TcpListener, TcpStream},
};

use crate::messages::{errors::Errors, reader::Reader, statemachine::StateMachine, Messages};

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
    let client = TcpStream::connect(&socket).await?;
    info!("Connected to peer");
    let sm = StateMachine::new_active();
    handle_connection(client, sm).await
}

async fn process_socket(socket: TcpStream) -> Result<(), std::io::Error> {
    info!("New connection from {:?}", socket.peer_addr());
    let sm = StateMachine::new_passive();
    handle_connection(socket, sm).await
}

async fn handle_connection(
    mut socket: TcpStream,
    mut sm: StateMachine,
) -> Result<(), std::io::Error> {
    let mut reader = Reader::new();
    let mut writer: Vec<u8> = Vec::new();
    loop {
        debug!("We are now at state {:?}", sm.state);
        if sm.should_close() {
            info!("We are done. Closing connection");
            return Ok(());
        }
        let ready = socket.ready(sm.get_interests()).await?;

        if ready.is_readable() {
            match reader.read(&mut socket).await {
                Ok(0) => {
                    info!("Connection closed by peer");
                    return Ok(());
                }
                Ok(n) => {
                    info!("read {} bytes", n);
                    let msg = sm.decode_message(&mut reader);
                    match msg {
                        Ok(Messages::ContactHeader(h)) => {
                            info!("Got contact header: {:?}", h);
                            sm.state_complete();
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

        if ready.is_writable() {
            if writer.is_empty() {
                sm.send_message(&mut writer);
            }
            match socket.write(&writer).await {
                Ok(0) => {
                    info!("Connection closed");
                    return Ok(());
                }
                Ok(n) => {
                    info!("wrote {} bytes", n);
                    if writer.len() == n {
                        writer.clear();
                        sm.state_complete();
                        info!("Write complete");
                    } else {
                        writer.drain(0..n);
                        info!("write incomplete. Trying again");
                    }
                }
                Err(_) => {}
            }
        }
    }
}
