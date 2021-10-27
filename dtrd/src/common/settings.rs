#[derive(Debug, Clone)]
pub struct Settings {
    pub my_node_id: String,
    pub tcpcl_listen_address: String,
    pub grpc_clientapi_address: String,
}
