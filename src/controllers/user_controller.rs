use crate::app_state::AppState;
use crate::models::user::User;
use crate::repository::user_repository;
use crate::services::auth::generate_enrollment_tokens;
use crate::utils::crypto_utils::verify_message_signature;
use axum::extract::Path;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::{extract::State, Json};
use serde::Deserialize;
use serde_json::json;
use uuid::Uuid;

#[derive(Deserialize)]
pub struct CreateUserBody {
    pub username: String,
}

#[derive(Deserialize)]
pub struct LoginUserBody {
    pub identity_pubkey: String,
    pub message: String,
    pub signature: String,
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

pub async fn login_user(
    State(state): State<AppState>,
    Json(payload): Json<LoginUserBody>,
) -> impl IntoResponse {

    if let Err(e) = verify_message_signature(
        &payload.identity_pubkey,
        &payload.message,
        &payload.signature,
    ) {
        eprintln!("Signature error: {}", e);
        return (
            StatusCode::UNAUTHORIZED,
            Json(json!({ "error": "Invalid signature" })),
        )
            .into_response();
    }
    match user_repository::find_user_by_pubkey(&state.pool, &payload.identity_pubkey).await {
        Ok(user) => {
            return (StatusCode::OK, Json(json!(user))).into_response();
        }
        Err(e) => {
            eprintln!("error : {}", e);
            return (
                StatusCode::UNAUTHORIZED,
                Json(json!({"error": "Unauthorized"})),
            )
                .into_response();
        }
    }
}

pub async fn get_user_by_id(
    State(state): State<AppState>,
    Path(user_id): Path<Uuid>,
) -> impl IntoResponse {
    match user_repository::find_user_by_id(&state.pool, &user_id).await {
        Ok(data) => return (StatusCode::OK, Json(data)).into_response(),
        Err(e) => {
            eprintln!("Error when fetching devices {}", e);
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
