use axum::{
    routing::{get, post},
    Router,
};

use crate::{app_state::AppState, controllers::federation_controller};

/// S2S routes (consumed by peer nodes, not by end clients).
///
/// /s2s/info is intentionally unauthenticated: it is the bootstrap endpoint
/// that lets an unknown peer fetch this node's public key before they have a
/// cached record. All other /s2s/* routes require the AuthenticatedNode
/// extractor (Ed25519 signature verification + nonce claim).
///
/// The client-facing federated proxy (GET /users/federated/:address/keys)
/// is included here for co-location but uses the normal AuthenticatedDevice
/// extractor, not AuthenticatedNode.
pub fn routes() -> Router<AppState> {
    Router::new()
        // ── Public ──────────────────────────────────────────────────────────
        .route("/s2s/info", get(federation_controller::node_info))
        // ── S2S (node-to-node, AuthenticatedNode required inside handler) ───
        .route(
            "/s2s/users/{username}/devices",
            get(federation_controller::get_user_devices),
        )
        .route(
            "/s2s/users/{username}/keys",
            get(federation_controller::get_user_keys),
        )
        .route("/s2s/sessions", post(federation_controller::receive_session))
        .route("/s2s/messages", post(federation_controller::receive_messages))
        .route("/s2s/ack", post(federation_controller::receive_ack))
        // ── Client-facing federated proxy ────────────────────────────────────
        .route(
            "/s2s/users/federated/{address}/keys",
            get(federation_controller::federated_keys),
        )
}
