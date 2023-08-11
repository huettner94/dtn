use actix::prelude::*;
use url::Url;

#[derive(Message)]
#[rtype(result = "")]
pub struct ConnectRemote {
    pub url: Url,
}

#[derive(Message)]
#[rtype(result = "")]
pub struct DisconnectRemote {
    pub url: Url,
}
