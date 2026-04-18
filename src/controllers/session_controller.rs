use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use serde::Deserialize;
use serde_json::json;
use uuid::Uuid;

use crate::app_state::AppState;
use crate::federation::{client::FederationClient, parse_federated_address};
use crate::middlewares::auth::AuthenticatedDevice;
use crate::models::federation::{S2sSessionInit, S2sSessionPayload};
use crate::repository::{session_repository, user_repository};

use super::messages_controller::resolve_node;

#[derive(Debug, Deserialize)]
pub struct SessionInit {
    pub recipient_device_id: Uuid,
    pub ephemeral_pubkey: String,
    pub sender_prekey_pub: String,
    pub otpk_used: String,
    pub ciphertext: String,
}

#[derive(Debug, Deserialize)]
pub struct CreateSessionBody {
    pub recipient_user_id: Uuid,
    /// Optional federated address for cross-node session initiation.
    /// Format: "username@node-host" (e.g. "bob@node-b.hushnet.net").
    #[serde(default)]
    pub recipient_user_address: Option<String>,
    pub sessions_init: Vec<SessionInit>,
}

#[derive(Debug, Deserialize)]
pub struct ConfirmSessionBody {
    pub pending_session_id: Uuid,
    pub sender_device_id: Uuid,
    pub receiver_device_id: Uuid,
}

pub async fn create_session(
    State(state): State<AppState>,
    AuthenticatedDevice(sender): AuthenticatedDevice,
    Json(payload): Json<CreateSessionBody>,
) -> Result<impl IntoResponse, (StatusCode, &'static str)> {
    // ── Federated path ────────────────────────────────────────────────────────
    if let Some(ref addr) = payload.recipient_user_address {
        if let Some((username, node_id)) = parse_federated_address(addr) {
            if node_id != state.this_node_id {
                return handle_federated_session(&state, &sender, &payload, username, node_id)
                    .await
                    .map_err(|_| (StatusCode::BAD_GATEWAY, "failed to forward session"));
            }
        }
    }

    // ── Local path (unchanged) ────────────────────────────────────────────────
    if sender.user_id == payload.recipient_user_id {
        return Err((StatusCode::BAD_REQUEST, "Cannot create session with self"));
    }

    let tx = state.pool.begin().await.map_err(|_| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Failed to start transaction",
        )
    })?;

    for init in &payload.sessions_init {
        session_repository::create_pending_session(
            &state.pool,
            &sender.id,
            &init.recipient_device_id,
            &init.ephemeral_pubkey,
            &init.sender_prekey_pub,
            &init.otpk_used,
            &init.ciphertext,
        )
        .await
        .map_err(|e| {
            eprintln!("Failed to insert pending session: {e:#?}");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to insert pending session",
            )
        })?;
    }

    tx.commit()
        .await
        .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "Failed to commit"))?;

    Ok((StatusCode::CREATED, Json(json!({ "status": "ok" }))).into_response())
}

/// Forward an X3DH session initiation to a peer node.
///
/// Node A signs and POSTs to Node B's /s2s/sessions endpoint. Node B inserts
/// the pending session records and delivers a WebSocket notification to the
/// recipient client. Node A returns 202 Accepted immediately.
async fn handle_federated_session(
    state: &AppState,
    sender: &crate::models::device::Devices,
    payload: &CreateSessionBody,
    to_username: &str,
    target_node_id: &str,
) -> Result<axum::response::Response, ()> {
    let sender_username =
        match user_repository::find_user_by_id(&state.pool, &sender.user_id).await {
            Ok(Some(u)) => u.username,
            _ => return Err(()),
        };

    let node = match resolve_node(state, target_node_id).await {
        Ok(n) => n,
        Err(resp) => return Ok(resp),
    };

    let s2s_payload = S2sSessionPayload {
        from_federated_address: format!("{}@{}", sender_username, state.this_node_id),
        from_device_id: sender.id,
        from_identity_pubkey: sender.identity_pubkey.clone(),
        to_user: to_username.to_string(),
        sessions_init: payload
            .sessions_init
            .iter()
            .map(|i| S2sSessionInit {
                recipient_device_id: i.recipient_device_id,
                ephemeral_pubkey: i.ephemeral_pubkey.clone(),
                sender_prekey_pub: i.sender_prekey_pub.clone(),
                otpk_used: i.otpk_used.clone(),
                ciphertext: i.ciphertext.clone(),
            })
            .collect(),
    };

    let fed_client = FederationClient::new(
        state.http_client.clone(),
        state.node_keys.clone(),
        state.this_node_id.clone(),
    );

    if let Err(e) = fed_client.forward_session(&node.api_url, &s2s_payload).await {
        eprintln!("[federated session] forward failed: {e}");
        return Ok((
            StatusCode::BAD_GATEWAY,
            Json(json!({"error": "failed to reach target node"})),
        )
            .into_response());
    }

    Ok((StatusCode::ACCEPTED, Json(json!({"status": "forwarded"}))).into_response())
}

pub async fn get_pending_sessions_handler(
    State(state): State<AppState>,
    AuthenticatedDevice(device): AuthenticatedDevice,
) -> Result<impl IntoResponse, (StatusCode, &'static str)> {
    let sessions =
        session_repository::get_pending_sessions(&state.pool, AuthenticatedDevice(device))
            .await
            .map_err(|_| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Failed to fetch pending sessions",
                )
            })?;

    if sessions.is_empty() {
        return Ok((StatusCode::OK, Json(json!({ "sessions": [] }))));
    }

    Ok((StatusCode::OK, Json(json!({ "sessions": sessions }))))
}

pub async fn confirm_session(
    State(state): State<AppState>,
    AuthenticatedDevice(device): AuthenticatedDevice,
    Json(payload): Json<ConfirmSessionBody>,
) -> Result<impl IntoResponse, (StatusCode, &'static str)> {
    let pending_session = session_repository::get_pending_session_by_id(
        &state.pool,
        &payload.pending_session_id,
        &device.id,
    )
    .await
    .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "DB error"))?;

    match pending_session {
        Some(ps) => ps,
        None => {
            return Err((
                StatusCode::NOT_FOUND,
                "Pending session not found or not owned by device",
            ))
        }
    };

    let chat_id = session_repository::get_or_create_chat_id(
        &state.pool,
        &payload.sender_device_id,
        &payload.receiver_device_id,
    )
    .await
    .map_err(|e| {
        eprintln!("Error: {e:#?}");
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Failed to get or create chat",
        )
    })?;

    session_repository::insert_or_update_session(
        &state.pool,
        &chat_id,
        &payload.sender_device_id,
        &payload.receiver_device_id,
    )
    .await
    .map_err(|e| {
        eprintln!("Error: {e:#?}");
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Failed to insert session",
        )
    })?;

    session_repository::delete_pending_session(&state.pool, &payload.pending_session_id)
        .await
        .map_err(|_| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to delete pending session",
            )
        })?;

    Ok((
        StatusCode::CREATED,
        Json(json!({ "status": "session confirmed" })),
    ))
}
