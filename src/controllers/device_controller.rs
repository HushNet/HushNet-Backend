use axum::extract::Path;
use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use serde::Deserialize;
use serde_json::json;
use uuid::Uuid;

use crate::app_state::AppState;
use crate::models::device::OneTimePrekeys;
use crate::models::device::SignedPreKey;
use crate::repository::device_repository;
use crate::repository::enrollment_token_repository::add_used_token;
use crate::repository::enrollment_token_repository::enrollment_token_exists;
use crate::services::auth::verify_enrollment_token;
use crate::utils::crypto_utils::verify_signed_prekey_signature;

#[derive(Deserialize, Debug)]
pub struct CreateDeviceBody {
    pub user_id: Uuid,
    pub identity_pubkey: String,
    pub prekey_pubkey: String,
    pub signed_prekey: SignedPreKey,
    pub one_time_prekeys: Vec<OneTimePrekeys>,
    pub device_label: String,
    pub push_token: String,
    pub enrollment_token: String,
}

pub async fn get_devices_for_user(
    State(state): State<AppState>,
    Path(user_id): Path<Uuid>,
) -> impl IntoResponse {
    match device_repository::get_devices_by_user_id(&state.pool, &user_id).await {
        Ok(data) => (StatusCode::OK, Json(data)).into_response(),
        Err(e) => {
            eprintln!("Error when fetching devices {}", e);
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

pub async fn create_device(
    State(state): State<AppState>,
    Json(payload): Json<CreateDeviceBody>,
) -> impl IntoResponse {
    println!("{:?}", payload);

    match enrollment_token_exists(&state.pool, &payload.enrollment_token).await {
        Ok(true) => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(json!({
                    "error": "Wrong or expired enrollment token for user"
                })),
            )
                .into_response()
        }
        Ok(false) => {
            println!("user enrollment token is still valid")
        }
        Err(e) => {
            eprintln!("Database error checking enrollment token: {:?}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "Database error"})),
            )
                .into_response();
        }
    }
    let user: Option<Uuid> = verify_enrollment_token(&payload.enrollment_token, &state.jwt_secret);
    // Check if the signature for the keys are valid

    if let Err(error) = verify_signed_prekey_signature(
        &payload.identity_pubkey,
        &payload.signed_prekey.key,
        &payload.signed_prekey.signature,
    ) {
        eprintln!("Signature check failed : {}", error);
        return (
            StatusCode::UNAUTHORIZED,
            Json(json!({"error": "Signature check failed."})),
        )
            .into_response();
    }
    if let Some(id) = user {
        if id != payload.user_id {
            return (
                StatusCode::UNAUTHORIZED,
                Json(json!({
                    "error": "Wrong or expired enrollment token for user"
                })),
            )
                .into_response();
        }
    };
    let prekeys_json = json!(payload
        .one_time_prekeys
        .iter()
        .map(|p| &p.key)
        .collect::<Vec<_>>());
    match device_repository::create_device(
        &state.pool,
        &payload.user_id,
        &payload.identity_pubkey,
        &payload.prekey_pubkey,
        &payload.signed_prekey.key,
        &payload.signed_prekey.signature,
        &prekeys_json,
        &payload.device_label,
        &payload.push_token,
    )
    .await
    {
        Ok(device) => match add_used_token(&state.pool, &payload.enrollment_token).await {
            Ok(_) => (StatusCode::CREATED, Json(device)).into_response(),
            Err(e) => {
                eprintln!("Error when adding enrollment token to used : {}", e);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({"error" : "Error creating device"})),
                )
                    .into_response()
            }
        },
        Err(e) => {
            if let sqlx::Error::Database(db_err) = &e {
                if db_err.code().as_deref() == Some("23503") {
                    return (
                        StatusCode::BAD_REQUEST,
                        Json(json!({"error" : "User ID does not exist"})),
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

pub async fn get_user_keys(
    State(state): State<AppState>,
    Path(user_id): Path<Uuid>,
) -> impl IntoResponse {
    match device_repository::get_device_bundle(&state.pool, &user_id).await {
        Ok(bundle) => (StatusCode::OK, Json(bundle)).into_response(),
        Err(_) => (
            StatusCode::NOT_FOUND,
            Json(json!({ "error": "User not found or has no active devices" })),
        )
            .into_response(),
    }
}

pub async fn get_user_for_device(
    State(state): State<AppState>,
    Path(device_id): Path<Uuid>,
) -> impl IntoResponse {
    match device_repository::get_user_for_device(&state.pool, &device_id).await {
        Ok(user) => (StatusCode::OK, Json(user)).into_response(),
        Err(_) => (
            StatusCode::NOT_FOUND,
            Json(json!({ "error": "User not found for device" })),
        )
            .into_response(),
    }
}
