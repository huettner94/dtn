use std::env;

#[derive(Debug, Clone)]
pub struct Settings {
    pub my_node_id: String,
    pub tcpcl_listen_address: String,
    pub grpc_clientapi_address: String,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            my_node_id: "dtn://defaultnodeid".into(),
            tcpcl_listen_address: "[::1]:4556".into(),
            grpc_clientapi_address: "[::1]:50051".into(),
        }
    }
}

impl Settings {
    pub fn from_env() -> Self {
        let mut settings = Settings::default();
        if let Ok(setting) = env::var("NODE_ID") {
            settings.my_node_id = setting;
        };
        if let Ok(setting) = env::var("TCPCL_LISTEN_ADDRESS") {
            settings.tcpcl_listen_address = setting;
        };
        if let Ok(setting) = env::var("GRPC_CLIENTAPI_ADDRESS") {
            settings.grpc_clientapi_address = setting;
        };
        settings
    }
}
