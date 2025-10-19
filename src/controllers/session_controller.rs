use axum::extract::Path;
use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use serde::Deserialize;
use serde_json::json;
use sqlx::PgPool;
use uuid::Uuid;

use crate::app_state::AppState;
use crate::middlewares::auth::AuthenticatedDevice;
use crate::repository::{device_repository, session_repository};

#[derive(Debug, serde::Deserialize)]
pub struct CreateSessionBody {
    pub recipient_user_id: Uuid,
    pub ephemeral_pubkey: String,
    pub ciphertext: String,
}

pub async fn create_session(
    State(state): State<AppState>,
    AuthenticatedDevice(sender): AuthenticatedDevice,
    Json(payload): Json<CreateSessionBody>,
) -> Result<impl IntoResponse, (StatusCode, &'static str)> {
    // 1️⃣ Récupère tous les devices du user cible
    let devices = device_repository::get_devices_by_user_id(&state.pool, &payload.recipient_user_id)
        .await
        .map_err(|_| (StatusCode::NOT_FOUND, "User devices not found"))?;

    // 2️⃣ Crée une session pour chaque device
    for device in devices {
        session_repository::create_session(
            &state.pool,
            &sender.id,
            &device.id,
            &payload.ephemeral_pubkey,
            &payload.ciphertext,
        )
        .await
        .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "Failed to create session"))?;
    }

    Ok((StatusCode::CREATED, Json(json!({"status": "ok"}))))
}
