use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use serde_json::json;

use crate::{
    app_state::AppState, middlewares::auth::AuthenticatedDevice, repository::chat_repository,
};

pub async fn get_all_chats(
    State(state): State<AppState>,
    AuthenticatedDevice(sender): AuthenticatedDevice,
) -> impl IntoResponse {
    match chat_repository::get_chats_for_device(&state.pool, AuthenticatedDevice(sender)).await {
        Ok(data) => (StatusCode::OK, Json(data)).into_response(),
        Err(e) => {
            eprintln!("Error when fetching chats {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "error": "Internal server error"
                })),
            )
                .into_response()
        }
    }
}
