use std::{collections::HashMap, io, net::SocketAddr};

use bp7::endpoint::Endpoint;
use log::{error, info, warn};
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
    common::{messages::Shutdown, settings::Settings},
    converganceagent::messages::{AgentForwardBundle, CLRegisterNode, CLUnregisterNode},
};

use actix::{prelude::*, spawn};

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
}

impl StreamHandler<Transfer> for TCPCLSessionAgent {
    fn handle(&mut self, item: Transfer, ctx: &mut Self::Context) {
        todo!()
    }
}

impl Handler<AgentForwardBundle> for TCPCLSessionAgent {
    type Result = ();

    fn handle(&mut self, msg: AgentForwardBundle, ctx: &mut Self::Context) -> Self::Result {
        todo!()
    }
}

impl Handler<Shutdown> for TCPCLSessionAgent {
    type Result = ();

    fn handle(&mut self, _msg: Shutdown, ctx: &mut Self::Context) -> Self::Result {
        match self.close_channel.take() {
            Some(c) => {
                if let Err(_) = c.send(()) {
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
        match Endpoint::new(&item.peer_endpoint.as_ref().unwrap()) {
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

/*
}

impl Daemon {


    async fn connect_remote(&mut self, socket: SocketAddr) {
        match TCPCLSession::connect(
            socket,
            self.settings.my_node_id.clone(),
            self.tls_settings.clone(),
        )
        .await
        {
            Ok(sess) => self.process_socket(sess).await,
            Err(e) => {
                error!("Error connecting to requested remote {}: {:?}", socket, e);
                if let Err(e) = self.convergance_agent_sender.as_ref().unwrap().send(
                    ConverganceAgentRequest::CLUnregisterNode {
                        node: None,
                        url: format!("tcpcl://{}", &socket),
                    },
                ) {
                    error!("Error sending message to convergance agent: {:?}", e);
                }
            }
        };
    }

    async fn disconnect_remote(&mut self, socket: SocketAddr) {
        match self.close_channels.remove(&socket) {
            Some(cc) => {
                if let Err(_) = cc.send(()) {
                    error!("Error sending message to convergance agent");
                };
            }
            None => {}
        }
    }


    async fn process_socket(&mut self, mut sess: TCPCLSession) {
        let close_channel = sess.get_close_channel();
        self.close_channels
            .insert(sess.get_connection_info().peer_address, close_channel);

        let send_channel = sess.get_send_channel();



        let mut transfer_receiver = sess.get_receive_channel();
        let receiver_convergane_agent_sender =
            self.convergance_agent_sender.as_ref().unwrap().clone();
        tokio::spawn(async move {
            loop {
                match transfer_receiver.recv().await {
                    Some(t) => {
                        info!("Received transfer id {}", t.id);
                        match t.data.try_into() {
                            Ok(bundle) => {
                                let (forward_response_sender, forward_response_receiver) =
                                    oneshot::channel();
                                match receiver_convergane_agent_sender.send(
                                    ConverganceAgentRequest::CLForwardBundle {
                                        bundle,
                                        responder: forward_response_sender,
                                    },
                                ) {
                                    Ok(_) => {
                                        match forward_response_receiver.await {
                                            Ok(_) => {}
                                            Err(_) => {
                                                // TODO: don't drop it here but send and error to remote
                                                warn!("Error saving received bundle. Dropping now");
                                            }
                                        }
                                    }
                                    Err(e) => {
                                        // TODO: don't drop it here but send and error to remote
                                        warn!(
                                        "Error sending received bundle to Convergance Agent. Dropping now: {:?}",
                                        e
                                    );
                                    }
                                };
                            }
                            Err(e) => {
                                warn!("Remote send invalid bundle. Dropping...: {:?}", e);
                            }
                        }
                    }
                    None => break,
                }
            }
        });


        self.tcpcl_sessions.push(jh);
    }
}

fn get_bundle_sender(
    send_channel: mpsc::Sender<(Vec<u8>, oneshot::Sender<Result<(), TransferSendErrors>>)>,
) -> mpsc::Sender<crate::converganceagent::messages::AgentForwardBundle> {
    let (bundle_sender, mut bundle_receiver) = mpsc::channel::<AgentForwardBundle>(32);

    tokio::spawn(async move {
        loop {
            match bundle_receiver.recv().await {
                Some(afb) => {
                    match afb.bundle.get_bundle().try_into() {
                        Ok(bundle_data) => {
                            let (status_sender, status_receiver) = oneshot::channel();
                            match send_channel.send((bundle_data, status_sender)).await {
                                Ok(_) => {}
                                Err(e) => {
                                    error!("Error sending bundle to tcpcl connection. {}", e);
                                }
                            }
                            match status_receiver.await {
                                Ok(status) => {
                                    match status {
                                        Ok(_) => {
                                            info!("Bundle successfully sent");
                                            if let Err(_) = afb.responder.send(Ok(())) {
                                                error!("Error notifying bpa of successfull sent bundle");
                                            };
                                            continue;
                                        }
                                        Err(e) => {
                                            error!("Error sending bundle because of {:?}", e);
                                        }
                                    }
                                }
                                Err(e) => {
                                    error!("We could not receive a bundle status: {:?}", e);
                                }
                            }
                        }
                        Err(e) => {
                            error!("Error converting bundle to bytes: {:?}", e);
                        }
                    };
                    if let Err(_) = afb.responder.send(Err(())) {
                        error!("Error notifying bpa of failed sent bundle");
                    };
                }
                None => return,
            }
        }
    });

    return bundle_sender;
}
*/
