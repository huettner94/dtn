use openssl::{
    pkey::{PKey, Private},
    x509::X509,
};

pub mod connection_info;
pub mod errors;
pub mod session;
pub mod transfer;
mod v4;

#[derive(Clone)]
pub struct TLSSettings {
    private_key: PKey<Private>,
    certificate: X509,
    trusted_certs: Vec<X509>,
}

impl TLSSettings {
    pub fn new(private_key: PKey<Private>, certificate: X509, trusted_certs: Vec<X509>) -> Self {
        Self {
            private_key,
            certificate,
            trusted_certs,
        }
    }
}
