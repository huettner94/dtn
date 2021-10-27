use std::net::SocketAddr;

#[derive(Debug)]
pub enum TCPCLAgentRequest {
    ConnectRemote { socket: SocketAddr },
}
