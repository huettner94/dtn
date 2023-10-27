use prost::Message;

mod bitswap {
    include!(concat!(env!("OUT_DIR"), "/bitswap.rs"));
}

pub struct BitswapServer {
    client: dtrd_client::Client,
}

impl BitswapServer {
    pub fn new(client: dtrd_client::Client) -> Self {
        BitswapServer { client }
    }

    pub async fn run(mut self) -> Result<(), dtrd_client::error::Error> {
        let request = bitswap::Message {
            wantlist: Some(bitswap::message::Wantlist {
                entries: Vec::new(),
                full: true,
            }),
            payload: Vec::new(),
            block_presences: Vec::new(),
            pending_bytes: 0,
        };
        println!("{:?}", request);
        let request_data = request.encode_to_vec();
        println!("{:x?}", request_data);
        self.client
            .submit_bundle("dtn://dtrd.int.eurador.de/replistore", 60, &request_data)
            .await?;
        Ok(())
    }
}
