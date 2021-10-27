use std::net::SocketAddr;

#[derive(Debug, Clone)]
pub struct ConnectionInfo {
    pub peer_endpoint: Option<String>,
    pub peer_address: SocketAddr,
}
