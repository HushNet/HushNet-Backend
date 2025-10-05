use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use serde::Deserialize;
use serde_json::json;
use sqlx::PgPool;
use uuid::Uuid;

use crate::repository::device_repository;

#[derive(Deserialize)]
pub struct CreateDeviceBody {
    pub user_id: Uuid,
    pub identity_pubkey: String,
    pub device_label: String,
    pub push_token: String
}


pub async fn create_device(
    State(pool): State<PgPool>,
    Json(payload): Json<CreateDeviceBody>,
) -> impl IntoResponse {
    match device_repository::create_device(&pool, &payload.user_id, &payload.identity_pubkey, &payload.device_label, &payload.push_token).await {
        Ok(device) => (StatusCode::CREATED, Json(device)).into_response(),
        Err(e) => {
            if let sqlx::Error::Database(db_err) = &e {
                if db_err.code().as_deref() == Some("23503") {
                    return (
                        StatusCode::BAD_REQUEST,
                        Json(json!({"error" : "User ID does not exist"}))
                    )
                    .into_response();
                }
            }
            eprintln!("Database error : {:?}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error" : "Error creating device"})),
            )
            .into_response()
        }
    }
}