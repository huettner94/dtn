use bp7::endpoint::Endpoint;
use log::error;
use tokio::sync::{mpsc, oneshot};

use super::messages::RoutingAgentRequest;

pub async fn get_next_hop(
    sender: &mpsc::Sender<RoutingAgentRequest>,
    target: Endpoint,
) -> Option<Endpoint> {
    let (responder_sender, responder_receiver) = oneshot::channel();
    match sender
        .send(RoutingAgentRequest::GetNextHop {
            target,
            responder: responder_sender,
        })
        .await
    {
        Ok(_) => match responder_receiver.await {
            Ok(e) => e,
            Err(e) => {
                error!("Error receiving request from routing agent: {:?}", e);
                None
            }
        },
        Err(e) => {
            error!("Error sending request to routing agent: {:?}", e);
            None
        }
    }
}
