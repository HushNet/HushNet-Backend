// src/middlewares/node_auth.rs
//
// Authenticates inbound S2S requests from peer nodes.
//
// Every request to a /s2s/* endpoint (except /s2s/info) must carry four
// headers that together prove the request was sent by the node that owns the
// private key registered at the central registry:
//
//   X-Node-ID        — canonical node identifier ("node-a.hushnet.net")
//   X-Timestamp      — Unix seconds as a decimal string
//   X-Nonce          — random 16-byte value, base64-encoded
//   X-Node-Signature — Ed25519 signature, base64-encoded
//
// Canonical string (UTF-8, signed verbatim, fields separated by "\n"):
//
//   {HTTP_METHOD}\n{path}\n{timestamp}\n{nonce}
//
// The path component is the request URI path only (no scheme or host), so
// that the canonical string is independent of which domain name the caller
// used to reach this node.
//
// Verification sequence
// ---------------------
// 1. Reject if |now − timestamp| > 60 s.
// 2. Look up the peer's FederationNode record (DB cache → registry fallback).
// 3. Reject if the node is flagged is_blocked.
// 4. Verify the Ed25519 signature over the canonical string.
// 5. Atomically claim the (node_id, nonce) pair in used_node_nonces; reject
//    if the pair was already present (replay attack).
//
// On success the FederationNode record is inserted into request Extensions so
// that handlers can access it with `Extension<FederationNode>`.

use crate::{
    app_state::AppState, models::federation::FederationNode, repository::federation_repository,
};
use axum::{
    extract::FromRequestParts,
    http::{request::Parts, StatusCode},
};
use base64::{engine::general_purpose::STANDARD as B64, Engine};
use ed25519_dalek::{Signature, Verifier, VerifyingKey};

/// Extractor that validates the four S2S authentication headers and returns the
/// authenticated peer's FederationNode record on success.
///
/// Usage in a handler:
/// ```
/// pub async fn my_handler(
///     AuthenticatedNode(peer): AuthenticatedNode,
///     ...
/// ) -> impl IntoResponse { ... }
/// ```
pub struct AuthenticatedNode(pub FederationNode);

impl FromRequestParts<AppState> for AuthenticatedNode {
    type Rejection = (StatusCode, String);

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let node_id = header_str(&parts.headers, "X-Node-ID")?;
        let ts_str = header_str(&parts.headers, "X-Timestamp")?;
        let nonce = header_str(&parts.headers, "X-Nonce")?;
        let sig_b64 = header_str(&parts.headers, "X-Node-Signature")?;

        // ── 1. timestamp check ───────────────────────────────────────────────
        let now = chrono::Utc::now().timestamp();
        let ts: i64 = ts_str.parse().map_err(|_| {
            (
                StatusCode::BAD_REQUEST,
                "X-Timestamp must be an integer".into(),
            )
        })?;
        if (now - ts).abs() > 60 {
            return Err((
                StatusCode::UNAUTHORIZED,
                "timestamp outside 60-second window".into(),
            ));
        }

        // ── 2. peer public key lookup (DB cache → registry fallback) ─────────
        let node = resolve_peer(state, &node_id).await?;

        // ── 3. blocked check ─────────────────────────────────────────────────
        if node.is_blocked {
            return Err((StatusCode::FORBIDDEN, "node is blocked".into()));
        }

        // ── 4. signature verification ────────────────────────────────────────
        let path = parts
            .uri
            .path_and_query()
            .map(|pq| pq.as_str())
            .unwrap_or("/");
        let canonical = format!("{}\n{}\n{}\n{}", parts.method.as_str(), path, ts_str, nonce);

        let sig_bytes: [u8; 64] = B64
            .decode(&sig_b64)
            .map_err(|_| (StatusCode::BAD_REQUEST, "bad signature base64".into()))?
            .try_into()
            .map_err(|_| (StatusCode::BAD_REQUEST, "signature must be 64 bytes".into()))?;
        let sig = Signature::from_bytes(&sig_bytes);

        let vk_bytes: [u8; 32] = B64
            .decode(&node.public_key_b64)
            .map_err(|_| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "bad cached peer pubkey".into(),
                )
            })?
            .try_into()
            .map_err(|_| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "peer pubkey must be 32 bytes".into(),
                )
            })?;
        let vk = VerifyingKey::from_bytes(&vk_bytes).map_err(|_| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "invalid peer pubkey".into(),
            )
        })?;

        vk.verify(canonical.as_bytes(), &sig)
            .map_err(|_| (StatusCode::UNAUTHORIZED, "invalid node signature".into()))?;

        // ── 5. nonce claim (replay prevention) ───────────────────────────────
        let fresh = federation_repository::claim_nonce(&state.pool, &node_id, &nonce)
            .await
            .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "db error".into()))?;
        if !fresh {
            return Err((StatusCode::UNAUTHORIZED, "replayed nonce".into()));
        }

        Ok(AuthenticatedNode(node))
    }
}

fn header_str(
    headers: &axum::http::HeaderMap,
    name: &'static str,
) -> Result<String, (StatusCode, String)> {
    headers
        .get(name)
        .and_then(|v| v.to_str().ok())
        .map(String::from)
        .ok_or_else(|| (StatusCode::UNAUTHORIZED, format!("missing header: {name}")))
}

/// Look up a peer's FederationNode, falling back to the central registry if the
/// node is not yet cached locally.
///
/// On a successful registry fetch, the node record is upserted into
/// federation_nodes so subsequent requests use the local cache.
async fn resolve_peer(
    state: &AppState,
    node_id: &str,
) -> Result<FederationNode, (StatusCode, String)> {
    if let Some(node) = federation_repository::get_federation_node(&state.pool, node_id)
        .await
        .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "db error".into()))?
    {
        return Ok(node);
    }

    // Cache miss: ask the central registry.
    let url = format!("{}/api/registry/nodes/{}", state.registry_url, node_id);
    let resp = state
        .http_client
        .get(&url)
        .send()
        .await
        .map_err(|_| {
            (
                StatusCode::SERVICE_UNAVAILABLE,
                "registry unreachable".into(),
            )
        })?
        .error_for_status()
        .map_err(|_| {
            (
                StatusCode::UNAUTHORIZED,
                "peer node not found in registry".into(),
            )
        })?
        .json::<serde_json::Value>()
        .await
        .map_err(|_| {
            (
                StatusCode::BAD_GATEWAY,
                "malformed registry response".into(),
            )
        })?;

    let api_url = resp["api_url"].as_str().ok_or((
        StatusCode::BAD_GATEWAY,
        "registry response missing api_url".into(),
    ))?;
    let pubkey = resp["public_key_b64"].as_str().ok_or((
        StatusCode::BAD_GATEWAY,
        "registry response missing public_key_b64".into(),
    ))?;

    let node = federation_repository::upsert_federation_node(&state.pool, node_id, api_url, pubkey)
        .await
        .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "db error".into()))?;

    Ok(node)
}
