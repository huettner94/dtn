use url::Url;

#[derive(Debug, Clone)]
pub struct ConnectionInfo {
    pub peer_endpoint: Option<String>,
    pub peer_url: Url,
    pub max_bundle_size: Option<u64>,
}
