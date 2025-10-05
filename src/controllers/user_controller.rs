use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::{Json, extract::State};
use serde::Deserialize;
use serde_json::json;
use crate::repository::user_repository;
use crate::models::user::User;
use sqlx::PgPool;


#[derive(Deserialize)]
pub struct CreateUserBody {
    pub username: String,
}

pub async fn list_users(State(pool): State<PgPool>) -> Json<Vec<User>> {
    let users = user_repository::get_all_users(&pool).await.unwrap();
    Json(users)
}

pub async fn create_user(
    State(pool): State<PgPool>,
    Json(payload): Json<CreateUserBody>,
) -> impl IntoResponse {
    match user_repository::create_user(&pool, &payload.username).await {
        Ok(user) => (StatusCode::CREATED, Json(json!(user))),
        Err(e) => {
            eprintln!("Database error: {:?}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": "Error creating user" })),
            )
        }
    }
}
