use crate::{
    app_state::AppState,
    federation::{client::FederationClient, parse_federated_address},
    middlewares::auth::AuthenticatedDevice,
    models::{
        federation::{S2sDevicePayload, S2sMessagePayload},
        message::OutgoingMessage,
    },
    repository::{
        federation_repository, message_repository::{fetch_pending_messages, insert_message},
        user_repository,
    },
};
use axum::{
    extract::{Json, State},
    http::StatusCode,
    response::IntoResponse,
};
use serde_json::json;
use uuid::Uuid;

pub async fn send_message(
    State(state): State<AppState>,
    AuthenticatedDevice(device): AuthenticatedDevice,
    Json(msg): Json<OutgoingMessage>,
) -> impl IntoResponse {
    let from_user_id: Uuid = device.user_id;

    // ── Federated path ────────────────────────────────────────────────────────
    // When to_user_address is present and points to a different node, bypass
    // local delivery entirely and queue the message for S2S forwarding.
    if let Some(ref addr) = msg.to_user_address {
        if let Some((username, node_id)) = parse_federated_address(addr) {
            if node_id != state.this_node_id {
                return handle_federated_message(
                    &state,
                    &device,
                    &msg,
                    from_user_id,
                    username,
                    node_id,
                )
                .await;
            }
        }
    }

    // ── Local delivery (existing path) ────────────────────────────────────────
    match insert_message(&state.pool, device.id, from_user_id, msg).await {
        Ok(()) => (StatusCode::OK, Json(json!({"success": "true"}))).into_response(),
        Err(e) => {
            eprintln!("Error inserting message: {e}");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "internal server error"})),
            )
                .into_response()
        }
    }
}

/// Build and queue a cross-node message for delivery to `username@node_id`.
///
/// Steps:
/// 1. Look up sender's username (needed for from_federated_address).
/// 2. Resolve target node from DB cache or central registry.
/// 3. Serialize the S2S payload and write to federation_outbox (durable).
/// 4. Spawn a task for immediate delivery; if it fails, the outbox worker
///    will retry on its next poll cycle.
/// 5. Return 202 Accepted — the client does not wait for Node B to respond.
async fn handle_federated_message(
    state: &AppState,
    device: &crate::models::device::Devices,
    msg: &OutgoingMessage,
    from_user_id: Uuid,
    to_username: &str,
    target_node_id: &str,
) -> axum::response::Response {
    // Look up sender's username for the federated address.
    let sender_username = match user_repository::find_user_by_id(&state.pool, &from_user_id).await
    {
        Ok(Some(u)) => u.username,
        _ => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "cannot resolve sender identity"})),
            )
                .into_response()
        }
    };

    // Resolve the target node (DB → registry).
    let node = match resolve_node(state, target_node_id).await {
        Ok(n) => n,
        Err(resp) => return resp,
    };

    let s2s_payload = S2sMessagePayload {
        logical_msg_id: msg.logical_msg_id.clone(),
        from_federated_address: format!("{}@{}", sender_username, state.this_node_id),
        from_device_id: device.id,
        from_identity_pubkey: device.identity_pubkey.clone(),
        to_user: to_username.to_string(),
        payloads: msg
            .payloads
            .iter()
            .map(|p| S2sDevicePayload {
                to_device_id: p.to_device_id,
                header: p.header.clone(),
                ciphertext: p.ciphertext.clone(),
            })
            .collect(),
    };

    let payload_json = match serde_json::to_value(&s2s_payload) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("Failed to serialize S2S payload: {e}");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "internal error"})),
            )
                .into_response();
        }
    };

    // Write to outbox for durability before attempting delivery.
    let outbox_id = match federation_repository::enqueue_outbox(
        &state.pool,
        target_node_id,
        &msg.logical_msg_id,
        &payload_json,
    )
    .await
    {
        Ok(id) => id,
        Err(e) => {
            eprintln!("Failed to enqueue outbox entry: {e}");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "internal error"})),
            )
                .into_response();
        }
    };

    // Spawn immediate delivery attempt; failures are handled by the outbox worker.
    let pool = state.pool.clone();
    let fed_client = FederationClient::new(
        state.http_client.clone(),
        state.node_keys.clone(),
        state.this_node_id.clone(),
    );
    let api_url = node.api_url.clone();

    tokio::spawn(async move {
        match fed_client.forward_messages(&api_url, &s2s_payload).await {
            Ok(_) => {
                let _ = federation_repository::mark_outbox_delivered(&pool, outbox_id).await;
            }
            Err(e) => {
                eprintln!("[federated send] immediate delivery failed, will retry: {e}");
                // Outbox worker schedules the next attempt automatically.
            }
        }
    });

    (StatusCode::ACCEPTED, Json(json!({"status": "queued"}))).into_response()
}

pub async fn get_pending_messages(
    State(state): State<AppState>,
    AuthenticatedDevice(device): AuthenticatedDevice,
) -> impl IntoResponse {
    match fetch_pending_messages(&state.pool, AuthenticatedDevice(device)).await {
        Ok(messages) => (StatusCode::OK, Json(messages)).into_response(),
        Err(e) => {
            eprintln!("Error fetching pending messages: {e}");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "internal server error"})),
            )
                .into_response()
        }
    }
}

// ── Shared helper ─────────────────────────────────────────────────────────────

/// Look up a FederationNode by node_id, falling back to the central registry
/// if the node is not yet cached locally.
pub(crate) async fn resolve_node(
    state: &AppState,
    node_id: &str,
) -> Result<crate::models::federation::FederationNode, axum::response::Response> {
    if let Ok(Some(n)) = federation_repository::get_federation_node(&state.pool, node_id).await {
        if n.is_blocked {
            return Err((
                StatusCode::FORBIDDEN,
                Json(json!({"error": "target node is blocked"})),
            )
                .into_response());
        }
        return Ok(n);
    }

    let url = format!("{}/api/registry/nodes/{}", state.registry_url, node_id);
    let resp = match state.http_client.get(&url).send().await {
        Ok(r) => r,
        Err(_) => {
            return Err((
                StatusCode::SERVICE_UNAVAILABLE,
                Json(json!({"error": "registry unreachable"})),
            )
                .into_response())
        }
    };

    if !resp.status().is_success() {
        return Err((
            StatusCode::NOT_FOUND,
            Json(json!({"error": "target node not found in registry"})),
        )
            .into_response());
    }

    let body = match resp.json::<serde_json::Value>().await {
        Ok(b) => b,
        Err(_) => {
            return Err((
                StatusCode::BAD_GATEWAY,
                Json(json!({"error": "malformed registry response"})),
            )
                .into_response())
        }
    };

    let api_url = body["api_url"].as_str().unwrap_or("");
    let pubkey = body["public_key_b64"].as_str().unwrap_or("");
    match federation_repository::upsert_federation_node(&state.pool, node_id, api_url, pubkey).await
    {
        Ok(n) => Ok(n),
        Err(e) => {
            eprintln!("Failed to upsert federation node: {e}");
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "internal error"})),
            )
                .into_response())
        }
    }
}
