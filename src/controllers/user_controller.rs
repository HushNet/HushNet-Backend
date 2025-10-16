use crate::app_state::AppState;
use crate::models::user::User;
use crate::repository::user_repository;
use crate::services::auth::generate_enrollment_tokens;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::{extract::State, Json};
use serde::Deserialize;
use serde_json::json;
use sqlx::PgPool;

#[derive(Deserialize)]
pub struct CreateUserBody {
    pub username: String,
}

pub async fn list_users(State(state): State<AppState>) -> Json<Vec<User>> {
    let users = user_repository::get_all_users(&state.pool).await.unwrap();
    Json(users)
}

pub async fn create_user(
    State(state): State<AppState>,
    Json(payload): Json<CreateUserBody>,
) -> impl IntoResponse {
    match user_repository::create_user(&state.pool, &payload.username).await {
        Ok(user) => {
            let token: String = generate_enrollment_tokens(&user.id, &state.jwt_secret);
            (
                StatusCode::CREATED,
                Json(json!({
                    "user": user,
                    "enrollment_token": token
                })),
            )
                .into_response()
        }
        Err(e) => {
            print!("Error creating user: {:?}", e);
            if let sqlx::Error::Database(db_err) = &e {
                if db_err.code().as_deref() == Some("23505") {
                    return (
                        StatusCode::CONFLICT,
                        Json(json!({"error" : "User already exists"})),
                    )
                        .into_response();
                }
            }

            eprintln!("Database error: {:?}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": "Error creating user" })),
            )
                .into_response()
        }
    }
}
