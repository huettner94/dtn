use std::{io::ErrorKind, net::SocketAddr, time::Duration};

use errors::ErrorType;
use log::{debug, info, warn};
use messages::sess_term::ReasonCode;
use session::TCPCLSession;
use tokio::{
    net::{TcpListener, TcpStream},
    time::sleep,
};

pub mod errors;
mod messages;
pub mod session;

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
    debug!("Now sleeping for 1 secs");
    sleep(Duration::from_secs(1)).await;
    sess.close(Some(ReasonCode::IdleTimeout)).await
}

async fn process_socket(socket: TcpStream) -> Result<(), std::io::Error> {
    info!("New connection from {}", socket.peer_addr()?);

    let mut sess = match TCPCLSession::new(socket).await {
        Ok(s) => s,
        Err(ErrorType::IOError(e)) => return Err(e),
        Err(ErrorType::TCPCLError(e)) => {
            warn!("Closing socket because of {:?}", e);
            return Ok(());
        }
    };

    match sess.wait().await {
        Ok(_) => Ok(()),
        Err(ErrorType::IOError(e)) => Err(e),
        Err(ErrorType::TCPCLError(e)) => {
            warn!("Closing socket because of {:?}", e);
            Ok(())
        }
    }
}
