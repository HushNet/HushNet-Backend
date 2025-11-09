use crate::{
    app_state::AppState,
    middlewares::auth::AuthenticatedDevice,
    models::message::OutgoingMessage,
    repository::message_repository::{fetch_pending_messages, insert_message},
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
    // Find user_id of sender
    let from_user_id: Uuid = device.user_id;

    match insert_message(&state.pool, device.id, from_user_id, msg).await {
        Ok(()) => {
            return (
                StatusCode::OK,
                Json(json!({
                    "success": "true"
                })),
            )
                .into_response()
        }
        Err(e) => {
            eprintln!("Error when inserting message {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "Internal server error"})),
            )
                .into_response();
        }
    }
}

pub async fn get_pending_messages(
    State(state): State<AppState>,
    AuthenticatedDevice(device): AuthenticatedDevice,
) -> impl IntoResponse {
    match fetch_pending_messages(&state.pool, AuthenticatedDevice(device)).await {
        Ok(messages) => return (StatusCode::OK, Json(messages)).into_response(),
        Err(e) => {
            eprintln!("Error when fetching pending messages {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "Internal server error"})),
            )
                .into_response();
        }
    }
}
