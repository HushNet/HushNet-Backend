// src/controllers/federation_controller.rs
//
// Handlers for the /s2s/* endpoint group.
//
// All handlers except node_info require the AuthenticatedNode extractor, which
// verifies the peer's Ed25519 signature before the handler body runs. Handlers
// receive the authenticated peer's FederationNode as a typed argument and can
// use it for logging or for constructing shadow records.
//
// Endpoint overview:
//
//   GET  /s2s/info                  — public, no auth
//   GET  /s2s/users/:username/devices — return device list (auth required)
//   GET  /s2s/users/:username/keys  — return prekey bundle, consume OTPK (auth)
//   POST /s2s/sessions              — accept forwarded X3DH init (auth)
//   POST /s2s/messages              — accept forwarded ciphertexts (auth)
//   POST /s2s/ack                   — delivery acknowledgment (auth)
//
// Plus one client-facing federated lookup:
//
//   GET  /users/federated/:address/keys — proxy prekey bundle from remote node

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde_json::json;

use crate::{
    app_state::AppState,
    federation::{client::FederationClient, parse_federated_address},
    middlewares::node_auth::AuthenticatedNode,
    models::federation::{
        NodeInfo, S2sAck, S2sMessagePayload, S2sSessionPayload,
    },
    repository::{device_repository, federation_repository, message_repository, session_repository},
};

// ─── GET /s2s/info ───────────────────────────────────────────────────────────

/// Return this node's public identity.
///
/// No authentication required. Peers call this during bootstrapping to obtain
/// the public key before they have a cached entry for this node. The caller
/// should cross-check the returned key against the central registry to guard
/// against a MITM substituting a different key.
pub async fn node_info(State(state): State<AppState>) -> impl IntoResponse {
    let info = NodeInfo {
        node_id: state.this_node_id.clone(),
        api_url: state.this_api_url.clone(),
        public_key_b64: state.node_keys.public_b64.clone(),
        protocol_version: "0.0.1",
    };
    (StatusCode::OK, Json(info))
}

// ─── GET /s2s/users/:username/devices ────────────────────────────────────────

/// Return the device list for a local user.
///
/// Used by a peer to enumerate recipient devices before building per-device
/// encrypted payloads. Only local (non-shadow) users are served; requests for
/// shadow users (home_node_id IS NOT NULL) return 404.
pub async fn get_user_devices(
    State(state): State<AppState>,
    AuthenticatedNode(_peer): AuthenticatedNode,
    Path(username): Path<String>,
) -> impl IntoResponse {
    let user_id = match federation_repository::get_local_user_id_by_username(&state.pool, &username)
        .await
    {
        Ok(Some(id)) => id,
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(json!({"error": "user not found or not local to this node"})),
            )
                .into_response()
        }
        Err(e) => {
            eprintln!("[s2s] db error in get_user_devices: {e}");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "internal error"})),
            )
                .into_response();
        }
    };

    match device_repository::get_devices_by_user_id(&state.pool, &user_id).await {
        Ok(devices) => (StatusCode::OK, Json(devices)).into_response(),
        Err(e) => {
            eprintln!("[s2s] db error fetching devices: {e}");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "internal error"})),
            )
                .into_response()
        }
    }
}

// ─── GET /s2s/users/:username/keys ───────────────────────────────────────────

/// Return the prekey bundle for a local user, consuming one OTPK per device.
///
/// Semantics are identical to GET /users/:id/keys on the client API. The
/// caller (Node A) receives the bundles, passes them to its client (Client A),
/// who uses them for X3DH key agreement without the servers ever seeing the
/// resulting shared secret.
///
/// If OTPKs are exhausted, the bundle is still returned (with an empty
/// one_time_prekeys list). The caller signals this to its client via the
/// `otpk_available` flag so the client can decide whether to proceed with
/// SPK-only X3DH or wait for replenishment.
pub async fn get_user_keys(
    State(state): State<AppState>,
    AuthenticatedNode(_peer): AuthenticatedNode,
    Path(username): Path<String>,
) -> impl IntoResponse {
    let user_id = match federation_repository::get_local_user_id_by_username(&state.pool, &username)
        .await
    {
        Ok(Some(id)) => id,
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(json!({"error": "user not found or not local to this node"})),
            )
                .into_response()
        }
        Err(e) => {
            eprintln!("[s2s] db error in get_user_keys: {e}");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "internal error"})),
            )
                .into_response();
        }
    };

    match device_repository::get_device_bundle(&state.pool, &user_id).await {
        Ok(bundle) => (StatusCode::OK, Json(bundle)).into_response(),
        Err(e) => {
            eprintln!("[s2s] db error fetching key bundle: {e}");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "internal error"})),
            )
                .into_response()
        }
    }
}

// ─── POST /s2s/sessions ──────────────────────────────────────────────────────

/// Accept a forwarded X3DH session initiation from a peer node.
///
/// Inserts the pending session(s) into this node's pending_sessions table.
/// The PostgreSQL trigger notify_new_pending_session fires automatically,
/// delivering a WebSocket event to the recipient client — the existing
/// real-time path requires no changes.
///
/// Shadow records for the sender (user + device) are upserted if they do not
/// already exist so that the FK constraints on pending_sessions are satisfied.
pub async fn receive_session(
    State(state): State<AppState>,
    AuthenticatedNode(peer): AuthenticatedNode,
    Json(payload): Json<S2sSessionPayload>,
) -> impl IntoResponse {
    // Upsert shadow user for the remote sender.
    let sender_local_id = match federation_repository::upsert_shadow_user(
        &state.pool,
        payload
            .from_federated_address
            .split('@')
            .next()
            .unwrap_or("unknown"),
        &payload.from_federated_address,
        peer.id,
    )
    .await
    {
        Ok(id) => id,
        Err(e) => {
            eprintln!("[s2s] shadow user upsert failed: {e}");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "internal error"})),
            )
                .into_response();
        }
    };

    if let Err(e) = federation_repository::upsert_shadow_device(
        &state.pool,
        payload.from_device_id,
        sender_local_id,
        &payload.from_identity_pubkey,
    )
    .await
    {
        eprintln!("[s2s] shadow device upsert failed: {e}");
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": "internal error"})),
        )
            .into_response();
    }

    // Insert one pending_session row per recipient device.
    for init in &payload.sessions_init {
        if let Err(e) = session_repository::create_pending_session(
            &state.pool,
            &payload.from_device_id,
            &init.recipient_device_id,
            &init.ephemeral_pubkey,
            &init.sender_prekey_pub,
            &init.otpk_used,
            &init.ciphertext,
        )
        .await
        {
            eprintln!("[s2s] pending session insert failed: {e}");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "failed to store pending session"})),
            )
                .into_response();
        }
    }

    (StatusCode::OK, Json(json!({"status": "ok"}))).into_response()
}

// ─── POST /s2s/messages ──────────────────────────────────────────────────────

/// Accept forwarded ciphertexts for a local recipient.
///
/// For each device payload:
/// - Deduplication check via the unique constraint (logical_msg_id, to_device_id).
///   Duplicate payloads (from outbox retries) are silently skipped; the 200
///   response is returned regardless so the sender stops retrying.
/// - The PostgreSQL trigger notify_new_message fires on every genuine insert,
///   pushing a WebSocket event to the recipient. No changes to the real-time
///   path are needed.
///
/// The returned S2sAck.status is "delivered" if at least one new row was
/// inserted, "duplicate" if all payloads were already present.
pub async fn receive_messages(
    State(state): State<AppState>,
    AuthenticatedNode(peer): AuthenticatedNode,
    Json(payload): Json<S2sMessagePayload>,
) -> impl IntoResponse {
    // Resolve the local recipient.
    let recipient_id =
        match federation_repository::get_local_user_id_by_username(&state.pool, &payload.to_user)
            .await
        {
            Ok(Some(id)) => id,
            Ok(None) => {
                return (
                    StatusCode::NOT_FOUND,
                    Json(json!({"error": "recipient not found or not local to this node"})),
                )
                    .into_response()
            }
            Err(e) => {
                eprintln!("[s2s] db error resolving recipient: {e}");
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({"error": "internal error"})),
                )
                    .into_response();
            }
        };

    // Upsert shadow records for the remote sender.
    let sender_local_id = match federation_repository::upsert_shadow_user(
        &state.pool,
        payload
            .from_federated_address
            .split('@')
            .next()
            .unwrap_or("unknown"),
        &payload.from_federated_address,
        peer.id,
    )
    .await
    {
        Ok(id) => id,
        Err(e) => {
            eprintln!("[s2s] shadow user upsert failed: {e}");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "internal error"})),
            )
                .into_response();
        }
    };

    if let Err(e) = federation_repository::upsert_shadow_device(
        &state.pool,
        payload.from_device_id,
        sender_local_id,
        &payload.from_identity_pubkey,
    )
    .await
    {
        eprintln!("[s2s] shadow device upsert failed: {e}");
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": "internal error"})),
        )
            .into_response();
    }

    // Find or create the local chat between shadow-sender and local-recipient.
    let chat_id = match federation_repository::get_or_create_direct_chat(
        &state.pool,
        sender_local_id,
        recipient_id,
    )
    .await
    {
        Ok(id) => id,
        Err(e) => {
            eprintln!("[s2s] get_or_create_direct_chat failed: {e}");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "internal error"})),
            )
                .into_response();
        }
    };

    // Insert each per-device ciphertext, skipping duplicates.
    let mut any_new = false;
    for dev in &payload.payloads {
        match message_repository::insert_federated_message(
            &state.pool,
            &payload.logical_msg_id,
            chat_id,
            sender_local_id,
            payload.from_device_id,
            recipient_id,
            dev.to_device_id,
            &dev.header,
            &dev.ciphertext,
        )
        .await
        {
            Ok(true) => any_new = true,
            Ok(false) => {} // duplicate, silently skip
            Err(e) => {
                eprintln!("[s2s] message insert failed: {e}");
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({"error": "internal error"})),
                )
                    .into_response();
            }
        }
    }

    let ack = S2sAck {
        logical_msg_id: payload.logical_msg_id,
        status: if any_new {
            "delivered".into()
        } else {
            "duplicate".into()
        },
    };
    (StatusCode::OK, Json(ack)).into_response()
}

// ─── POST /s2s/ack ───────────────────────────────────────────────────────────

/// Receive a delivery acknowledgment from a peer.
///
/// The outbox worker already marks entries delivered when it gets a 2xx from
/// forward_messages, so this endpoint is not on the critical path. It exists
/// for peers that want to proactively signal delivery (e.g. after a delayed
/// WebSocket push) and for future monitoring use cases.
pub async fn receive_ack(
    State(state): State<AppState>,
    AuthenticatedNode(_peer): AuthenticatedNode,
    Json(ack): Json<S2sAck>,
) -> impl IntoResponse {
    if let Err(e) = federation_repository::mark_outbox_delivered_by_logical_id(
        &state.pool,
        &ack.logical_msg_id,
    )
    .await
    {
        eprintln!("[s2s] ack db error: {e}");
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": "internal error"})),
        )
            .into_response();
    }
    (StatusCode::OK, Json(json!({"status": "ack received"}))).into_response()
}

// ─── GET /users/federated/:address/keys ──────────────────────────────────────

/// Client-facing proxy: fetch the prekey bundle of a remote user.
///
/// `address` is the full federated address: "bob@node-b.hushnet.net".
///
/// This node (Node A) authenticates the request from Client A, resolves the
/// target node, makes an authenticated S2S call to Node B, and returns the
/// bundle verbatim. OTPKs are consumed on Node B; Node A never stores them.
///
/// If the target node's address matches this node's own node_id, the request
/// is redirected to the local GET /users/:id/keys path instead (handled in the
/// same response to avoid a network round-trip).
pub async fn federated_keys(
    State(state): State<AppState>,
    crate::middlewares::auth::AuthenticatedDevice(_device): crate::middlewares::auth::AuthenticatedDevice,
    Path(address): Path<String>,
) -> impl IntoResponse {
    let (username, node_id) = match parse_federated_address(&address) {
        Some(parts) => parts,
        None => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({"error": "invalid federated address, expected user@node"})),
            )
                .into_response()
        }
    };

    // If the address points to this node, serve locally.
    if node_id == state.this_node_id {
        let user_id =
            match federation_repository::get_local_user_id_by_username(&state.pool, username)
                .await
            {
                Ok(Some(id)) => id,
                Ok(None) => {
                    return (
                        StatusCode::NOT_FOUND,
                        Json(json!({"error": "user not found"})),
                    )
                        .into_response()
                }
                Err(e) => {
                    eprintln!("[federated_keys] local db error: {e}");
                    return (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(json!({"error": "internal error"})),
                    )
                        .into_response();
                }
            };
        return match device_repository::get_device_bundle(&state.pool, &user_id).await {
            Ok(bundle) => (StatusCode::OK, Json(bundle)).into_response(),
            Err(e) => {
                eprintln!("[federated_keys] local bundle error: {e}");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({"error": "internal error"})),
                )
                    .into_response()
            }
        };
    }

    // Remote node: look up in federation_nodes or fall back to registry.
    let node = match federation_repository::get_federation_node(&state.pool, node_id).await {
        Ok(Some(n)) => n,
        Ok(None) => {
            // Try to discover via registry.
            let url = format!("{}/api/registry/nodes/{}", state.registry_url, node_id);
            match state.http_client.get(&url).send().await {
                Ok(resp) if resp.status().is_success() => {
                    match resp.json::<serde_json::Value>().await {
                        Ok(body) => {
                            let api_url = body["api_url"].as_str().unwrap_or("");
                            let pubkey = body["public_key_b64"].as_str().unwrap_or("");
                            match federation_repository::upsert_federation_node(
                                &state.pool,
                                node_id,
                                api_url,
                                pubkey,
                            )
                            .await
                            {
                                Ok(n) => n,
                                Err(e) => {
                                    eprintln!("[federated_keys] upsert node failed: {e}");
                                    return (
                                        StatusCode::INTERNAL_SERVER_ERROR,
                                        Json(json!({"error": "internal error"})),
                                    )
                                        .into_response();
                                }
                            }
                        }
                        Err(_) => {
                            return (
                                StatusCode::BAD_GATEWAY,
                                Json(json!({"error": "malformed registry response"})),
                            )
                                .into_response();
                        }
                    }
                }
                _ => {
                    return (
                        StatusCode::NOT_FOUND,
                        Json(json!({"error": "target node not found in registry"})),
                    )
                        .into_response();
                }
            }
        }
        Err(e) => {
            eprintln!("[federated_keys] db error: {e}");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "internal error"})),
            )
                .into_response();
        }
    };

    if node.is_blocked {
        return (
            StatusCode::FORBIDDEN,
            Json(json!({"error": "target node is blocked"})),
        )
            .into_response();
    }

    let fed_client = FederationClient::new(
        state.http_client.clone(),
        state.node_keys.clone(),
        state.this_node_id.clone(),
    );

    match fed_client.fetch_peer_keys(&node.api_url, username).await {
        Ok(bundle) => (StatusCode::OK, Json(bundle)).into_response(),
        Err(e) => {
            eprintln!("[federated_keys] peer key fetch failed: {e}");
            (
                StatusCode::BAD_GATEWAY,
                Json(json!({"error": format!("peer returned error: {e}")})),
            )
                .into_response()
        }
    }
}
