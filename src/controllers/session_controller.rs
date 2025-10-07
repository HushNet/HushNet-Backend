use axum::extract::Path;
use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use serde::Deserialize;
use serde_json::json;
use sqlx::PgPool;
use uuid::Uuid;

use crate::middlewares::auth::AuthenticatedDevice;
use crate::repository::session_repository;

#[derive(Debug, serde::Deserialize)]
pub struct CreateSessionBody {
    pub recipient_device_id: Uuid,
    pub ephemeral_pubkey: String,
    pub ciphertext: String,
}

pub async fn create_session(
    State(pool): State<PgPool>,
    AuthenticatedDevice(sender): AuthenticatedDevice,
    Json(payload): Json<CreateSessionBody>,
) -> impl IntoResponse {
    match session_repository::create_session(
        &pool,
        &sender.id,
        &payload.recipient_device_id,
        &payload.ephemeral_pubkey,
        &payload.ciphertext,
    )
    .await
    {
        Ok(data) => return (StatusCode::OK, Json(data)).into_response(),
        Err(e) => {
            eprintln!("Error when creating session {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "error": "Internal server error"
                })),
            )
                .into_response();
        }
    }
}
