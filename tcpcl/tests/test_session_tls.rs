use std::{net::SocketAddrV4, pin::Pin, str::FromStr};

use openssl::{
    ssl::{Ssl, SslAcceptor, SslContext, SslMethod, SslVerifyMode},
    x509::store::X509StoreBuilder,
};
use tcpcl::{errors::ErrorType, session::TCPCLSession, TLSSettings};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
};
use tokio_openssl::SslStream;
use url::Url;

use crate::common::*;

mod common;

#[tokio::test]
async fn test_tls_connection_setup_client() -> Result<(), ErrorType> {
    let (server_key, server_cert) = tls::get_server_cert();
    let (client_key, client_cert) = tls::get_client_cert();
    let ca_server_cert = server_cert.clone();
    let ca_client_cert = client_cert.clone();

    let listener = TcpListener::bind(SocketAddrV4::from_str("127.0.0.1:0").unwrap()).await?;
    let addr = listener.local_addr()?;
    let jh = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();

        let mut buf: [u8; 100] = [0; 100];
        let len = socket.read(&mut buf).await.unwrap();
        assert_eq!(len, 6);
        assert_eq!(buf[0..6], CONTACT_HEADER_TLS);

        socket.write(&CONTACT_HEADER_TLS).await.unwrap();

        let mut x509_store_builder = X509StoreBuilder::new().unwrap();
        x509_store_builder.add_cert(ca_client_cert).unwrap();
        let mut ssl_acceptor = SslAcceptor::mozilla_modern_v5(SslMethod::tls_server()).unwrap();
        ssl_acceptor.set_cert_store(x509_store_builder.build());
        ssl_acceptor.set_private_key(&server_key).unwrap();
        ssl_acceptor.set_certificate(&server_cert).unwrap();
        ssl_acceptor.check_private_key().unwrap();
        ssl_acceptor.set_verify(SslVerifyMode::PEER | SslVerifyMode::FAIL_IF_NO_PEER_CERT);
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

    let url = Url::parse(&format!("tcpcl://{}", addr)).unwrap();
    let mut session = TCPCLSession::connect(
        url,
        "dtn://client".into(),
        Some(TLSSettings::new(
            client_key,
            client_cert,
            vec![ca_server_cert],
        )),
    )
    .await?;
    let established = session.get_established_channel();
    session.manage_connection().await.unwrap();
    jh.await.unwrap();

    let conn_info = established.await.unwrap();
    assert_eq!(conn_info.peer_endpoint.unwrap(), "dtn://server");

    Ok(())
}

#[tokio::test]
async fn test_tls_connection_setup_client_dns() -> Result<(), ErrorType> {
    let (server_key, server_cert) = tls::get_server_cert_dns();
    let (client_key, client_cert) = tls::get_client_cert();
    let ca_server_cert = server_cert.clone();
    let ca_client_cert = client_cert.clone();

    let listener = TcpListener::bind(SocketAddrV4::from_str("127.0.0.1:0").unwrap()).await?;
    let addr = listener.local_addr()?;
    let jh = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();

        let mut buf: [u8; 100] = [0; 100];
        let len = socket.read(&mut buf).await.unwrap();
        assert_eq!(len, 6);
        assert_eq!(buf[0..6], CONTACT_HEADER_TLS);

        socket.write(&CONTACT_HEADER_TLS).await.unwrap();

        let mut x509_store_builder = X509StoreBuilder::new().unwrap();
        x509_store_builder.add_cert(ca_client_cert).unwrap();
        let mut ssl_acceptor = SslAcceptor::mozilla_modern_v5(SslMethod::tls_server()).unwrap();
        ssl_acceptor.set_cert_store(x509_store_builder.build());
        ssl_acceptor.set_private_key(&server_key).unwrap();
        ssl_acceptor.set_certificate(&server_cert).unwrap();
        ssl_acceptor.check_private_key().unwrap();
        ssl_acceptor.set_verify(SslVerifyMode::PEER | SslVerifyMode::FAIL_IF_NO_PEER_CERT);
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

    let url = Url::parse(&format!("tcpcl://localhost:{}", addr.port())).unwrap();
    let mut session = TCPCLSession::connect(
        url,
        "dtn://client".into(),
        Some(TLSSettings::new(
            client_key,
            client_cert,
            vec![ca_server_cert],
        )),
    )
    .await?;
    let established = session.get_established_channel();
    session.manage_connection().await.unwrap();
    jh.await.unwrap();

    let conn_info = established.await.unwrap();
    assert_eq!(conn_info.peer_endpoint.unwrap(), "dtn://server");

    Ok(())
}

#[tokio::test]
async fn test_tls_connection_setup_server() -> Result<(), ErrorType> {
    let (server_key, server_cert) = tls::get_server_cert();
    let (client_key, client_cert) = tls::get_client_cert();
    let ca_cert = client_cert.clone();

    let listener = TcpListener::bind(SocketAddrV4::from_str("127.0.0.1:0").unwrap()).await?;
    let addr = listener.local_addr()?;
    let jh = tokio::spawn(async move {
        let mut client = TcpStream::connect(&addr).await.unwrap();
        client.write(&CONTACT_HEADER_TLS).await.unwrap();

        let mut buf: [u8; 100] = [0; 100];
        let len = client.read(&mut buf).await.unwrap();
        assert_eq!(len, 6);
        assert_eq!(buf[0..6], CONTACT_HEADER_TLS);

        let mut ssl_context_builder = SslContext::builder(SslMethod::tls_client()).unwrap();
        ssl_context_builder.set_private_key(&client_key).unwrap();
        ssl_context_builder.set_certificate(&client_cert).unwrap();
        ssl_context_builder.check_private_key().unwrap();
        let ssl_context = ssl_context_builder.build();
        let ssl = Ssl::new(&ssl_context).unwrap();
        let mut client = SslStream::new(ssl, client).unwrap();
        Pin::new(&mut client).connect().await.unwrap();

        client
            .write(&SESS_INIT_CLIENT_KEEPALIVE_NONE)
            .await
            .unwrap();

        let mut buf: [u8; 100] = [0; 100];
        let len = client.read(&mut buf).await.unwrap();
        assert_eq!(len, 37);
        assert_eq!(buf[0..37], SESS_INIT_SERVER);
    });

    let (socket, _) = listener.accept().await?;
    let mut session = TCPCLSession::new(
        socket,
        "dtn://server".into(),
        Some(TLSSettings::new(server_key, server_cert, vec![ca_cert])),
    )?;
    let established = session.get_established_channel();
    session.manage_connection().await.unwrap();
    jh.await.unwrap();

    let conn_info = established.await.unwrap();
    assert_eq!(conn_info.peer_endpoint.unwrap(), "dtn://client");

    Ok(())
}
