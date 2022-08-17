use bp7::bundle::Bundle;
use log::{debug, error, warn};

use super::{messages::*, StoredBundle};
use actix::prelude::*;

#[derive(Default)]
pub struct Daemon {
    bundles: Vec<StoredBundle>,
}

impl Actor for Daemon {
    type Context = Context<Self>;

    fn stopped(&mut self, ctx: &mut Context<Self>) {
        if self.bundles.len() != 0 {
            warn!(
                "BSA had {} bundles left over, they will be gone now.",
                self.bundles.len()
            );
        }
    }
}

impl actix::Supervised for Daemon {}

impl SystemService for Daemon {}

impl Handler<StoreBundle> for Daemon {
    type Result = Result<StoredBundle, ()>;

    fn handle(&mut self, msg: StoreBundle, ctx: &mut Context<Self>) -> Self::Result {
        let StoreBundle { bundle } = msg;
        debug!("Storing Bundle {:?} for later", bundle.primary_block);
        let res: Result<StoredBundle, ()> = match TryInto::<StoredBundle>::try_into(bundle) {
            Ok(sb) => {
                self.bundles.push(sb.clone());
                Ok(sb)
            }
            Err(e) => {
                error!("Error converting bundle to StoredBundle: {:?}", e);
                Err(())
            }
        };
        res
    }
}

impl Handler<DeleteBundle> for Daemon {
    type Result = ();

    fn handle(&mut self, msg: DeleteBundle, ctx: &mut Context<Self>) {
        let DeleteBundle { bundle } = msg;
        match self.bundles.iter().position(|b| b == bundle) {
            Some(idx) => {
                self.bundles.remove(idx);
            }
            None => {}
        }
    }
}

impl Handler<GetBundleForDestination> for Daemon {
    type Result = Result<Vec<StoredBundle>, String>;

    fn handle(&mut self, msg: GetBundleForDestination, ctx: &mut Context<Self>) -> Self::Result {
        let GetBundleForDestination { destination } = msg;
        let mut ret = Vec::new();
        for i in 0..self.bundles.len() {
            if self.bundles[i]
                .get_bundle()
                .primary_block
                .destination_endpoint
                == destination
            {
                ret.push(self.bundles[i].clone());
            }
        }
        debug!(
            "Returning {} bundles for destination {}",
            ret.len(),
            destination
        );
        Ok(ret)
    }
}

impl Handler<GetBundleForNode> for Daemon {
    type Result = Result<Vec<StoredBundle>, String>;

    fn handle(&mut self, msg: GetBundleForNode, ctx: &mut Context<Self>) -> Self::Result {
        let GetBundleForNode { destination } = msg;
        let mut ret = Vec::new();
        for i in 0..self.bundles.len() {
            if self.bundles[i]
                .get_bundle()
                .primary_block
                .destination_endpoint
                .matches_node(&destination)
            {
                ret.push(self.bundles[i].clone());
            }
        }
        debug!(
            "Returning {} bundles for destination {}",
            ret.len(),
            destination
        );
        Ok(ret)
    }
}

impl Handler<TryDefragmentBundle> for Daemon {
    type Result = Result<Option<StoredBundle>, ()>;

    fn handle(&mut self, msg: TryDefragmentBundle, ctx: &mut Context<Self>) -> Self::Result {
        let TryDefragmentBundle { bundle } = msg;
        let requested_primary_block = &bundle.get_bundle().primary_block;
        let mut i = 0;
        let mut fragments: Vec<Bundle> = Vec::new();
        while i < self.bundles.len() {
            if self.bundles[i]
                .bundle
                .primary_block
                .equals_ignoring_fragment_info(requested_primary_block)
            {
                fragments.push(self.bundles.remove(i).bundle.as_ref().clone()); // TODO: This is a full clone and probably bad
            } else {
                i += 1;
            }
        }
        let mut reassembled: Vec<StoredBundle> = Bundle::reassemble_bundles(fragments)
            .into_iter()
            .map(|frag| frag.try_into().expect("This can not happen"))
            .collect();
        if reassembled.len() == 1
            && reassembled[0]
                .get_bundle()
                .primary_block
                .fragment_offset
                .is_none()
        {
            Ok(Some(reassembled.drain(0..1).next().unwrap()))
        } else {
            self.bundles.append(&mut reassembled);
            Ok(None)
        }
    }
}
