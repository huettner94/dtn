use std::collections::HashMap;

use bp7::endpoint::Endpoint;
use log::{info, warn};
use tokio::sync::{mpsc, oneshot};
use tokio_util::sync::CancellationToken;

use crate::{
    bundleprotocolagent::messages::{NewClientConnected, SendBundle},
    common::settings::Settings,
    nodeagent::messages::{AddNode, ListNodes, Node, RemoveNode},
    routingagent::messages::{AddRoute, ListRoutes, RemoveRoute, RouteStatus, RouteType},
};

use super::messages::{
    AgentGetClient, ClientAddNode, ClientAddRoute, ClientListNodes, ClientListenBundles,
    ClientRemoveNode, ClientRemoveRoute, ClientSendBundle, ListenBundlesResponse,
};
use actix::prelude::*;

#[derive(Default)]
pub struct Daemon {
    clients: HashMap<Endpoint, (mpsc::Sender<ListenBundlesResponse>, CancellationToken)>,
}

impl Actor for Daemon {
    type Context = Context<Self>;

    fn stopped(&mut self, ctx: &mut Context<Self>) {
        info!("Closing all client agent channels");
        for (client_endpoint, client_sender) in self.clients.drain() {
            drop(client_sender);
            info!("Closed agent channel for {:?}", client_endpoint);
        }
    }
}

impl actix::Supervised for Daemon {}

impl SystemService for Daemon {}

impl Handler<ClientSendBundle> for Daemon {
    type Result = Result<(), ()>;

    fn handle(&mut self, msg: ClientSendBundle, ctx: &mut Context<Self>) -> Self::Result {
        let ClientSendBundle {
            destination,
            payload,
            lifetime,
        } = msg;
        crate::bundleprotocolagent::agent::Daemon::from_registry().send(SendBundle {
            destination,
            payload,
            lifetime,
        })
    }
}

impl Handler<ClientListenBundles> for Daemon {
    type Result = Result<(), String>;

    fn handle(&mut self, msg: ClientListenBundles, ctx: &mut Context<Self>) -> Self::Result {
        let ClientListenBundles {
            destination,
            responder,
            canceltoken,
        } = msg;
        let sender = self.bpa_sender.as_ref().unwrap();
        let (endpoint_local_response_sender, endpoint_local_response_receiver) =
            oneshot::channel::<bool>();

        let settings = Settings::from_env();
        let node_id = Endpoint::new(&settings.my_node_id).unwrap();

        if node_id != destination.get_node_endpoint() {
            warn!("User attempted to register with endpoint not bound here.");
            return Err("Endpoint invalid for this BundleProtocolAgent".to_string());
        }

        self.clients
            .insert(destination.clone(), (responder.clone(), canceltoken));
        crate::bundleprotocolagent::agent::Daemon::from_registry()
            .do_send(NewClientConnected { destination });

        Ok(())
    }
}

impl Handler<ClientListNodes> for Daemon {
    type Result = Vec<Node>;

    fn handle(&mut self, msg: ClientListNodes, ctx: &mut Context<Self>) -> Self::Result {
        let ClientListNodes { responder } = msg;
        crate::nodeagent::agent::Daemon::from_registry().send(ListNodes {})
    }
}

impl Handler<ClientAddNode> for Daemon {
    type Result = ();

    fn handle(&mut self, msg: ClientAddNode, ctx: &mut Context<Self>) -> Self::Result {
        let ClientAddNode { url } = msg;
        crate::nodeagent::agent::Daemon::from_registry().do_send(AddNode { url });
    }
}

impl Handler<ClientRemoveNode> for Daemon {
    type Result = ();

    fn handle(&mut self, msg: ClientRemoveNode, ctx: &mut Context<Self>) -> Self::Result {
        let ClientRemoveNode { url } = msg;
        crate::nodeagent::agent::Daemon::from_registry().do_send(RemoveNode { url });
    }
}

impl Handler<ClientListNodes> for Daemon {
    type Result = Vec<RouteStatus>;

    fn handle(&mut self, msg: ClientListNodes, ctx: &mut Context<Self>) -> Self::Result {
        crate::nodeagent::agent::Daemon::from_registry().send(ListRoutes {})
    }
}

impl Handler<ClientAddRoute> for Daemon {
    type Result = ();

    fn handle(&mut self, msg: ClientAddRoute, ctx: &mut Context<Self>) -> Self::Result {
        let ClientAddRoute { target, next_hop } = msg;
        crate::routingagent::agent::Daemon::from_registry().do_send(AddRoute {
            target,
            next_hop,
            route_type: RouteType::Static,
            max_bundle_size: None,
        });
    }
}

impl Handler<ClientRemoveRoute> for Daemon {
    type Result = ();

    fn handle(&mut self, msg: ClientRemoveRoute, ctx: &mut Context<Self>) -> Self::Result {
        let ClientRemoveRoute { target, next_hop } = msg;
        crate::routingagent::agent::Daemon::from_registry().do_send(RemoveRoute {
            target,
            next_hop,
            route_type: RouteType::Static,
        });
    }
}

impl Handler<AgentGetClient> for Daemon {
    type Result = Option<mpsc::Sender<ListenBundlesResponse>>;

    fn handle(&mut self, msg: AgentGetClient, ctx: &mut Context<Self>) -> Self::Result {
        let AgentGetClient { destination } = msg;
        match self.clients.get(&destination) {
            Some((sender, canceltoken)) => {
                if canceltoken.is_cancelled() {
                    info!("Client for endpoint {} already disconnected", destination);
                    self.clients.remove(&destination);
                    None
                } else {
                    Some(sender.clone())
                }
            }
            None => None,
        }
    }
}
