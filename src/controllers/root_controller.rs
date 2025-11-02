use crate::app_state::AppState;
use axum::{extract::State, response::IntoResponse, Json};
use serde_json::json;

pub async fn root(State(state): State<AppState>) -> impl IntoResponse {
    Json(json!({"message": "Welcome to the HushNet API"}))
}
