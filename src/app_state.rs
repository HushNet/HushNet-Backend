use std::sync::Arc;

use sqlx::PgPool;

use crate::utils::node_keys::NodeKeys;

#[derive(Clone)]
pub struct AppState {
    pub pool: PgPool,
    pub jwt_secret: String,
    /// This node's Ed25519 keypair, used to sign outbound S2S requests.
    pub node_keys: Arc<NodeKeys>,
    /// Canonical identifier for this node (e.g. "node-a.hushnet.net").
    /// Matches the node_id registered at the central registry.
    pub this_node_id: String,
    /// Base API URL for this node (e.g. "https://node-a.hushnet.net/api").
    /// Included in GET /s2s/info responses so peers know where to send requests.
    pub this_api_url: String,
    /// Central registry URL used for peer node discovery.
    pub registry_url: String,
    /// Shared HTTP client for outbound requests (registry lookups + S2S calls).
    /// reqwest::Client is Clone and internally reference-counted.
    pub http_client: reqwest::Client,
}
