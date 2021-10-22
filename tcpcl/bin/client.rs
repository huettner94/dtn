use std::net::{IpAddr, Ipv6Addr, SocketAddr};

use tcpcl::{connect, errors::ErrorType};

#[tokio::main]
async fn main() -> Result<(), ErrorType> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    let socket = SocketAddr::new(IpAddr::V6(Ipv6Addr::LOCALHOST), 4556);
    connect(socket).await
}
