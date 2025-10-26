use axum::extract::Path;
use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use base64::{engine::general_purpose, Engine as _};
use serde::Deserialize;
use serde_json::json;
use sqlx::PgPool;
use uuid::Uuid;

use crate::app_state::AppState;
use crate::middlewares::auth::AuthenticatedDevice;
use crate::repository::{device_repository, session_repository};

#[derive(Debug, serde::Deserialize)]
pub struct SessionInit {
    pub recipient_device_id: Uuid,
    pub ephemeral_pubkey: String,
    pub sender_prekey_pub: String,
    pub otpk_used: String,
    pub ciphertext: String,
}

#[derive(Debug, serde::Deserialize)]
pub struct CreateSessionBody {
    pub recipient_user_id: Uuid,
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
    if sender.user_id == payload.recipient_user_id {
        return Err((StatusCode::BAD_REQUEST, "Cannot create session with self"));
    }

    let mut tx = state.pool.begin()
        .await
        .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "Failed to start transaction"))?;

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
            // Print the underlying database error for debugging before mapping to a generic HTTP error
            eprintln!("Failed to insert pending session: {:#?}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, "Failed to insert pending session")
        })?;
    }

    tx.commit()
        .await
        .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "Failed to commit"))?;

    Ok((StatusCode::CREATED, Json(json!({ "status": "ok" }))))
}

pub async fn get_pending_sessions_handler(
    State(state): State<AppState>,
    AuthenticatedDevice(device): AuthenticatedDevice,
) -> Result<impl IntoResponse, (StatusCode, &'static str)> {
    let sessions = session_repository::get_pending_sessions(&state.pool, AuthenticatedDevice(device))
        .await
        .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "Failed to fetch pending sessions"))?;

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
    .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "DB error"))?;;

    match pending_session {
        Some(ps) => ps,
        None => return Err((StatusCode::NOT_FOUND, "Pending session not found or not owned by device")),
    };

    let chat_id = session_repository::get_or_create_chat_id(
        &state.pool,
        &payload.sender_device_id,
        &payload.receiver_device_id,
    )
    .await
    .map_err(|e| {
        eprintln!("Error {:#?}", e);
        (StatusCode::INTERNAL_SERVER_ERROR, "Failed to get or create chat")
    })?;
        

    session_repository::insert_or_update_session(
        &state.pool,
        &chat_id,
        &payload.sender_device_id,
        &payload.receiver_device_id
    )
    .await
    .map_err(|e| {
        eprintln!("Error {:#?}", e);
        (StatusCode::INTERNAL_SERVER_ERROR, "Failed to insert session")
    })?;
    session_repository::delete_pending_session(&state.pool, &payload.pending_session_id)
        .await
        .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "Failed to delete pending session"))?;

    Ok((StatusCode::CREATED, Json(json!({ "status": "session confirmed" }))))
}