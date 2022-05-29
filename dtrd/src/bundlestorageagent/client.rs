use bp7::bundle::Bundle;
use log::error;
use tokio::sync::{mpsc, oneshot};

use super::{messages::BSARequest, StoredBundle};

pub async fn store_bundle(
    sender: &mpsc::UnboundedSender<BSARequest>,
    bundle: Bundle,
) -> Result<StoredBundle, ()> {
    let (responder_sender, responder_receiver) = oneshot::channel();
    match sender.send(BSARequest::StoreBundle {
        bundle,
        responder: responder_sender,
    }) {
        Ok(_) => match responder_receiver.await {
            Ok(e) => e,
            Err(e) => {
                error!("Error receiving request from storage agent: {:?}", e);
                Err(())
            }
        },
        Err(e) => {
            error!("Error sending request to storage agent: {:?}", e);
            Err(())
        }
    }
}

pub async fn delete_bundle(sender: &mpsc::UnboundedSender<BSARequest>, bundle: StoredBundle) {
    match sender.send(BSARequest::DeleteBundle { bundle }) {
        Ok(_) => {}
        Err(e) => {
            error!("Error sending request to storage agent: {:?}", e);
        }
    }
}

pub async fn try_defragment_bundle(
    sender: &mpsc::UnboundedSender<BSARequest>,
    bundle: StoredBundle,
) -> Result<Option<StoredBundle>, ()> {
    let (responder_sender, responder_receiver) = oneshot::channel();
    match sender.send(BSARequest::TryDefragmentBundle {
        bundle,
        responder: responder_sender,
    }) {
        Ok(_) => match responder_receiver.await {
            Ok(e) => e,
            Err(e) => {
                error!("Error receiving request from storage agent: {:?}", e);
                Err(())
            }
        },
        Err(e) => {
            error!("Error sending request to storage agent: {:?}", e);
            Err(())
        }
    }
}
