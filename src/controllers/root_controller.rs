use crate::app_state::AppState;
use axum::{extract::State, response::IntoResponse, Json};
use serde_json::json;

pub async fn root(State(_state): State<AppState>) -> impl IntoResponse {
    Json(json!({"message": "Welcome to the HushNet API"}))
}

pub async fn health_check() -> impl IntoResponse {
    Json(json!({"status": "ok"}))
}
