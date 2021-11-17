use std::{net::SocketAddrV4, pin::Pin, str::FromStr};

use openssl::{
    asn1::Asn1Time,
    hash::MessageDigest,
    nid::Nid,
    pkey::{PKey, Private},
    rsa::Rsa,
    ssl::{Ssl, SslAcceptor, SslContext, SslMethod},
    x509::{X509Name, X509},
};
use tcpcl::{errors::ErrorType, session::TCPCLSession};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
};
use tokio_openssl::SslStream;

use crate::common::*;

mod common;

async fn get_server_certs() -> (PKey<Private>, X509) {
    let cert_rsa = Rsa::generate(1024).unwrap();
    let pkey = PKey::from_rsa(cert_rsa).unwrap();

    let mut name = X509Name::builder().unwrap();
    name.append_entry_by_nid(Nid::COMMONNAME, "example")
        .unwrap();
    let name = name.build();

    let mut builder = X509::builder().unwrap();
    builder.set_version(2).unwrap();
    builder.set_subject_name(&name).unwrap();
    builder.set_issuer_name(&name).unwrap();
    builder
        .set_not_before(&Asn1Time::days_from_now(0).unwrap())
        .unwrap();
    builder
        .set_not_after(&Asn1Time::days_from_now(365).unwrap())
        .unwrap();
    builder.set_pubkey(&pkey).unwrap();
    builder.sign(&pkey, MessageDigest::sha256()).unwrap();
    let x509 = builder.build();

    (pkey, x509)
}

#[tokio::test]
async fn test_tls_connection_setup_client() -> Result<(), ErrorType> {
    let (key, cert) = get_server_certs().await;
    let listener = TcpListener::bind(SocketAddrV4::from_str("127.0.0.1:0").unwrap()).await?;
    let addr = listener.local_addr()?;
    let jh = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();

        let mut buf: [u8; 100] = [0; 100];
        let len = socket.read(&mut buf).await.unwrap();
        assert_eq!(len, 6);
        assert_eq!(buf[0..6], CONTACT_HEADER_TLS);

        socket.write(&CONTACT_HEADER_TLS).await.unwrap();

        let mut ssl_acceptor = SslAcceptor::mozilla_modern_v5(SslMethod::tls_server()).unwrap();
        ssl_acceptor.set_private_key(&key).unwrap();
        ssl_acceptor.set_certificate(&cert).unwrap();
        ssl_acceptor.check_private_key().unwrap();
        let ssl_context = ssl_acceptor.build().into_context();
        let ssl = Ssl::new(&ssl_context).unwrap();
        let mut socket = SslStream::new(ssl, socket).unwrap();
        Pin::new(&mut socket).accept().await.unwrap();

        let mut buf: [u8; 100] = [0; 100];
        let len = socket.read(&mut buf).await.unwrap();
        assert_eq!(len, 37);
        assert_eq!(buf[0..37], SESS_INIT_CLIENT);

        socket.write(&SESS_INIT_SERVER).await.unwrap();
    });
    let ssl_context = SslContext::builder(SslMethod::tls_client())
        .unwrap()
        .build();
    let mut session = TCPCLSession::connect(addr, "dtn://client".into(), Some(ssl_context)).await?;
    let established = session.get_established_channel();
    session.manage_connection().await;
    jh.await.unwrap();

    let conn_info = established.await.unwrap();
    assert_eq!(conn_info.peer_endpoint.unwrap(), "dtn://server");

    Ok(())
}

#[tokio::test]
async fn test_tls_connection_setup_server() -> Result<(), ErrorType> {
    let (key, cert) = get_server_certs().await;
    let listener = TcpListener::bind(SocketAddrV4::from_str("127.0.0.1:0").unwrap()).await?;
    let addr = listener.local_addr()?;
    let jh = tokio::spawn(async move {
        let mut client = TcpStream::connect(&addr).await.unwrap();
        client.write(&CONTACT_HEADER_TLS).await.unwrap();

        let mut buf: [u8; 100] = [0; 100];
        let len = client.read(&mut buf).await.unwrap();
        assert_eq!(len, 6);
        assert_eq!(buf[0..6], CONTACT_HEADER_TLS);

        let ssl_context = SslContext::builder(SslMethod::tls_client())
            .unwrap()
            .build();
        let ssl = Ssl::new(&ssl_context).unwrap();
        let mut client = SslStream::new(ssl, client).unwrap();
        Pin::new(&mut client).connect().await.unwrap();

        client.write(&SESS_INIT_CLIENT).await.unwrap();

        let mut buf: [u8; 100] = [0; 100];
        let len = client.read(&mut buf).await.unwrap();
        assert_eq!(len, 37);
        assert_eq!(buf[0..37], SESS_INIT_SERVER);
    });
    let mut ssl_acceptor = SslAcceptor::mozilla_modern_v5(SslMethod::tls_server()).unwrap();
    ssl_acceptor.set_private_key(&key).unwrap();
    ssl_acceptor.set_certificate(&cert).unwrap();
    ssl_acceptor.check_private_key().unwrap();
    let ssl_context = ssl_acceptor.build().into_context();

    let (socket, _) = listener.accept().await?;
    let mut session = TCPCLSession::new(socket, "dtn://server".into(), Some(ssl_context))?;
    let established = session.get_established_channel();
    session.manage_connection().await;
    jh.await.unwrap();

    let conn_info = established.await.unwrap();
    assert_eq!(conn_info.peer_endpoint.unwrap(), "dtn://client");

    Ok(())
}
