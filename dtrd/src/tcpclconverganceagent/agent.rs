use std::{collections::HashMap, io, net::SocketAddr};

use bp7::endpoint::Endpoint;
use log::{debug, error, info, warn};
use openssl::{pkey::PKey, x509::X509};
use tcpcl::{
    connection_info::ConnectionInfo, errors::TransferSendErrors, session::TCPCLSession,
    transfer::Transfer, TLSSettings,
};
use tokio::{
    fs::File,
    io::AsyncReadExt,
    net::{TcpListener, TcpStream},
    sync::{broadcast, mpsc, oneshot},
    task::JoinHandle,
};
use tokio_stream::wrappers::ReceiverStream;

use crate::{
    bundlestorageagent::messages::StoreBundle,
    common::{messages::Shutdown, settings::Settings},
    converganceagent::messages::{
        AgentForwardBundle, CLRegisterNode, CLUnregisterNode, EventBundleForwarded,
        EventBundleForwardingFailed,
    },
};

use actix::{prelude::*, spawn};

use super::messages::{ConnectRemote, DisconnectRemote};

#[derive(Message)]
#[rtype(result = "")]
struct NewClientConnectedOnSocket {
    stream: TcpStream,
    address: SocketAddr,
}

pub async fn tcpcl_listener(
    mut shutdown: broadcast::Receiver<()>,
    _shutdown_complete_sender: mpsc::Sender<()>,
    tcpcl_server: Addr<TCPCLServer>,
) -> Result<JoinHandle<()>, io::Error> {
    let settings = Settings::from_env();

    let socket: SocketAddr = settings.tcpcl_listen_address.parse().unwrap();

    info!("Server listening on {}", socket);

    let listener = TcpListener::bind(&socket).await?;

    let joinhandle = spawn(async move {
        info!("Socket open, waiting for connection");
        loop {
            tokio::select! {
                conn = listener.accept() => {
                    match conn {
                        Ok((stream, address)) => {
                            tcpcl_server.do_send(NewClientConnectedOnSocket {stream, address});
                        },
                        Err(e) => {
                            error!("Something bad happend during accepting a connection for tcpcl: {:?}. Aborting...", &e);
                        }
                    }
                }
                _ = shutdown.recv() => {
                    info!("Received shutdown message, stopping the tcpcl socket");
                    break;
                }
            };
        }

        drop(listener); // implicitly closes the socket

        info!("TCPCL socket has shutdown. See you");
        // _shutdown_complete_sender is explicitly dropped here
    });
    Ok(joinhandle)
}

#[derive(Default)]
pub struct TCPCLServer {
    my_node_id: String,
    tls_config: Option<TLSSettings>,
    sessions: HashMap<SocketAddr, Addr<TCPCLSessionAgent>>,
}

impl Actor for TCPCLServer {
    type Context = Context<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        let settings = Settings::from_env();
        self.my_node_id = settings.my_node_id.clone();

        let fut = async move { TCPCLServer::load_tls_settings(&settings).await };
        fut.into_actor(self)
            .then(|res, act, ctx| {
                match res {
                    Ok(tls_config) => act.tls_config = tls_config,
                    Err(e) => {
                        error!("Error loading TLS configuration for tcpcl server: {}", e);
                        ctx.stop();
                    }
                }
                fut::ready(())
            })
            .wait(ctx);
    }
}

impl actix::Supervised for TCPCLServer {}

impl SystemService for TCPCLServer {}

impl Handler<NewClientConnectedOnSocket> for TCPCLServer {
    type Result = ();

    fn handle(
        &mut self,
        msg: NewClientConnectedOnSocket,
        _ctx: &mut Self::Context,
    ) -> Self::Result {
        let NewClientConnectedOnSocket { stream, address } = msg;
        info!("New client connected from {}", address);
        let session =
            match TCPCLSession::new(stream, self.my_node_id.clone(), self.tls_config.clone()) {
                Ok(s) => s,
                Err(e) => {
                    error!(
                        "Error handling new incoming connection: {:?}. Connection will be dropped",
                        e
                    );
                    return;
                }
            };

        let sessionagent = TCPCLSessionAgent::new(session);
        self.sessions.insert(address, sessionagent);
    }
}

impl Handler<ConnectRemote> for TCPCLServer {
    type Result = ();

    fn handle(&mut self, msg: ConnectRemote, ctx: &mut Self::Context) -> Self::Result {
        let ConnectRemote { address } = msg;
        let fut = TCPCLSession::connect(address, self.my_node_id.clone(), self.tls_config.clone());
        fut.into_actor(self)
            .then(move |ret, act, _ctx| {
                match ret {
                    Ok(session) => {
                        let sessionagent = TCPCLSessionAgent::new(session);
                        act.sessions.insert(address, sessionagent);
                    }
                    Err(e) => {
                        error!("Error connecting to remote tcpcl: {:?}", e);
                        crate::converganceagent::agent::Daemon::from_registry().do_send(
                            CLUnregisterNode {
                                url: format!("tcpcl://{}", &address),
                                node: None,
                            },
                        );
                    }
                }
                fut::ready(())
            })
            .wait(ctx);
    }
}

impl Handler<DisconnectRemote> for TCPCLServer {
    type Result = ();

    fn handle(&mut self, msg: DisconnectRemote, _ctx: &mut Self::Context) -> Self::Result {
        let DisconnectRemote { address } = msg;
        match self.sessions.remove(&address) {
            Some(sess) => {
                sess.do_send(Shutdown {});
            }
            None => {}
        }
    }
}

impl Handler<Shutdown> for TCPCLServer {
    type Result = ();

    fn handle(&mut self, _msg: Shutdown, _ctx: &mut Self::Context) -> Self::Result {
        for (_, session) in self.sessions.drain() {
            session.do_send(Shutdown {});
        }
    }
}

impl TCPCLServer {
    async fn load_tls_settings(settings: &Settings) -> Result<Option<TLSSettings>, std::io::Error> {
        if settings.tcpcl_certificate_path.is_some()
            && settings.tcpcl_key_path.is_some()
            && settings.tcpcl_trusted_certs_path.is_some()
        {
            let mut certificate_file =
                File::open(settings.tcpcl_certificate_path.as_ref().unwrap()).await?;
            let mut certificate_data = Vec::new();
            certificate_file.read_to_end(&mut certificate_data).await?;
            let certificate = X509::from_der(&certificate_data)?;

            let mut key_file = File::open(settings.tcpcl_key_path.as_ref().unwrap()).await?;
            let mut key_data = Vec::new();
            key_file.read_to_end(&mut key_data).await?;
            let key = PKey::private_key_from_der(&key_data)?;

            let mut trusted_file =
                File::open(settings.tcpcl_trusted_certs_path.as_ref().unwrap()).await?;
            let mut trusted_data = Vec::new();
            trusted_file.read_to_end(&mut trusted_data).await?;
            let trusted = X509::from_der(&trusted_data)?;
            info!("Starting TCPCL agent with TLS Support");
            return Ok(Some(TLSSettings::new(key, certificate, vec![trusted])));
        }
        info!("Starting TCPCL agent without TLS Support");
        Ok(None)
    }
}

struct TCPCLSessionAgent {
    close_channel: Option<oneshot::Sender<()>>,
    send_channel: mpsc::Sender<(Vec<u8>, oneshot::Sender<Result<(), TransferSendErrors>>)>,
}

impl Actor for TCPCLSessionAgent {
    type Context = Context<Self>;

    fn stopping(&mut self, _ctx: &mut Self::Context) -> Running {
        // This isn't what it looks like :)
        // when we finish establishing the tcpcl session we get the information
        // about the connection using StreamHandler<ConnectionInfo>. However
        // the stream is closed afterwards. Actix treats this as a reason to stop
        // the whole actor. With this pice of code we only stop when we actually
        // want to or if the connection got finished by the remote (send_channel)
        if self.close_channel.is_some() && !self.send_channel.is_closed() {
            Running::Continue
        } else {
            Running::Stop
        }
    }
}

impl StreamHandler<Transfer> for TCPCLSessionAgent {
    fn handle(&mut self, item: Transfer, ctx: &mut Self::Context) {
        match item.data.try_into() {
            Ok(bundle) => {
                let transferid = item.id;
                crate::bundlestorageagent::agent::Daemon::from_registry()
                    .send(StoreBundle { bundle })
                    .into_actor(self)
                    .then(move |res, _act, _ctx| {
                        match res.unwrap() {
                            Ok(_) => {debug!("Successfully received transfer {}", transferid)},
                            Err(_) => {error!("Error storing transfered bundle with id {}. It would have been better if we did not ack it", transferid)},
                        };
                        fut::ready(())})
                    .spawn(ctx);
            }
            Err(e) => {
                error!("Error deserializing bundle from remote: {:?}", e);
            }
        };
    }
}

impl Handler<AgentForwardBundle> for TCPCLSessionAgent {
    type Result = ();

    fn handle(&mut self, msg: AgentForwardBundle, ctx: &mut Self::Context) -> Self::Result {
        let AgentForwardBundle { bundle, responder } = msg;
        let (result_sender, result_receiver) = oneshot::channel();

        let bundle_data = match bundle.get_bundle().try_into() {
            Ok(bundle_data) => bundle_data,
            Err(e) => {
                error!("Error serializing bundle: {:?}", e);
                return;
            }
        };
        let bundle_endpoint = bundle
            .get_bundle()
            .primary_block
            .destination_endpoint
            .clone();

        let channel = self.send_channel.clone();
        let fut = async move { channel.send((bundle_data, result_sender)).await };
        fut.into_actor(self)
            .then(|res, _act, ctx| {
                if let Err(_) = res {
                    error!("Error sending bundle to tcpcl connection. Killing the connection");
                    ctx.stop();
                } else {
                    let listener = async move {
                        match result_receiver.await {
                            Ok(send_result) => match send_result {
                                Ok(_) => {
                                    responder
                                        .send(EventBundleForwarded {
                                            endpoint: bundle_endpoint,
                                            bundle,
                                        })
                                        .await
                                        .unwrap();
                                }
                                Err(e) => {
                                    error!("Error during sending of bundle: {:?}", e);
                                    crate::bundleprotocolagent::agent::Daemon::from_registry()
                                        .send(EventBundleForwardingFailed {
                                            endpoint: bundle_endpoint,
                                            bundle,
                                        })
                                        .await
                                        .unwrap();
                                }
                            },
                            Err(_) => {
                                error!("Error during receiving bundle status results.");
                            }
                        }
                    };
                    tokio::spawn(listener); // We drop the join handle here because we never need to access it again
                }
                fut::ready(())
            })
            .wait(ctx);
    }
}

impl Handler<Shutdown> for TCPCLSessionAgent {
    type Result = ();

    fn handle(&mut self, _msg: Shutdown, ctx: &mut Self::Context) -> Self::Result {
        match self.close_channel.take() {
            Some(c) => {
                if c.send(()).is_err() {
                    warn!("Error sending shutdown message to tcpcl session. Forcing it to die by stopping us");
                    ctx.stop();
                }
            }
            None => {}
        };
    }
}

impl StreamHandler<ConnectionInfo> for TCPCLSessionAgent {
    fn handle(&mut self, item: ConnectionInfo, ctx: &mut Self::Context) {
        match Endpoint::new(item.peer_endpoint.as_ref().unwrap()) {
            Some(node) => {
                crate::converganceagent::agent::Daemon::from_registry().do_send(CLRegisterNode {
                    url: format!("tcpcl://{}", item.peer_address),
                    node,
                    max_bundle_size: item
                        .max_bundle_size
                        .expect("We must have a bundle size if we are connected"),
                    sender: ctx.address().recipient(),
                });
            }
            None => {
                warn!(
                    "Peer send invalid id '{}'.",
                    item.peer_endpoint.as_ref().unwrap()
                );
                ctx.stop();
            }
        }
    }
}

impl TCPCLSessionAgent {
    fn new(mut session: TCPCLSession) -> Addr<Self> {
        TCPCLSessionAgent::create(|ctx| {
            ctx.add_stream(ReceiverStream::new(session.get_receive_channel()));

            let established_channel = session.get_established_channel();
            ctx.add_stream(async_stream::stream! {yield established_channel.await.unwrap();});

            let close_channel = session.get_close_channel();
            let send_channel = session.get_send_channel();

            let fut = async move {
                if let Err(e) = session.manage_connection().await {
                    warn!("Connection closed with error: {:?}", e);
                }
                let ci = session.get_connection_info();
                let node = match ci.peer_endpoint {
                    Some(endpoint) => Endpoint::new(&endpoint),
                    None => None,
                };
                crate::converganceagent::agent::Daemon::from_registry().do_send(CLUnregisterNode {
                    url: format!("tcpcl://{}", ci.peer_address),
                    node,
                });
            };
            tokio::spawn(fut); // We drop the join handle here because we never need to access it again

            TCPCLSessionAgent {
                close_channel: Some(close_channel),
                send_channel,
            }
        })
    }
}
