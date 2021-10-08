use std::net::{IpAddr, Ipv4Addr, SocketAddr};

use tcpcl::connect;

#[tokio::main]
async fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    let socket = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8080);
    connect(socket).await;
}
