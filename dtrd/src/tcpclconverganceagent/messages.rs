use actix::prelude::*;
use std::net::SocketAddr;

#[derive(Message)]
#[rtype(result = "")]
pub struct ConnectRemote {
    socket: SocketAddr,
}

#[derive(Message)]
#[rtype(result = "")]
pub struct DisonnectRemote {
    socket: SocketAddr,
}
