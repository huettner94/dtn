use actix::prelude::*;
use std::net::SocketAddr;

#[derive(Message)]
#[rtype(result = "")]
pub struct ConnectRemote {
    pub address: SocketAddr,
}

#[derive(Message)]
#[rtype(result = "")]
pub struct DisconnectRemote {
    pub address: SocketAddr,
}
