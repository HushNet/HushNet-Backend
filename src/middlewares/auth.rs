// src/middlewares/auth.rs
use crate::{app_state::AppState, models::device::Devices, repository::device_repository};
use axum::{
    async_trait,
    extract::FromRequestParts,
    http::{request::Parts, StatusCode},
};
use base64::{engine::general_purpose::STANDARD as b64, Engine};
use ed25519_dalek::{Signature, Verifier, VerifyingKey};

pub struct AuthenticatedDevice(pub Devices);

#[async_trait]
impl FromRequestParts<AppState> for AuthenticatedDevice {
    type Rejection = (StatusCode, String);

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        // Read Headers
        let ik_b64 = parts
            .headers
            .get("X-Identity-Key")
            .and_then(|v| v.to_str().ok())
            .ok_or((StatusCode::UNAUTHORIZED, "Missing X-Identity-Key".into()))?;
        let sig_b64 = parts
            .headers
            .get("X-Signature")
            .and_then(|v| v.to_str().ok())
            .ok_or((StatusCode::UNAUTHORIZED, "Missing X-Signature".into()))?;
        let ts = parts
            .headers
            .get("X-Timestamp")
            .and_then(|v| v.to_str().ok())
            .ok_or((StatusCode::UNAUTHORIZED, "Missing X-Timestamp".into()))?;

        // anti-replay
        let now = chrono::Utc::now().timestamp();
        let ts_i64: i64 = ts.parse().unwrap_or(0);
        if (now - ts_i64).abs() > 30 {
            return Err((StatusCode::UNAUTHORIZED, "Expired timestamp".into()));
        }

        let sig_bytes: [u8; 64] = b64
            .decode(sig_b64)
            .map_err(|_| (StatusCode::BAD_REQUEST, "Bad signature b64".into()))?
            .try_into()
            .map_err(|_| (StatusCode::BAD_REQUEST, "Signature must be 64 bytes".into()))?;

        let sig = Signature::from_bytes(&sig_bytes);
        let vk_bytes = b64
            .decode(ik_b64)
            .map_err(|_| (StatusCode::BAD_REQUEST, "Bad pubkey b64".into()))?;
        let vk_arr: [u8; 32] = vk_bytes
            .try_into()
            .map_err(|_| (StatusCode::BAD_REQUEST, "Bad pubkey length".into()))?;
        let vk = VerifyingKey::from_bytes(&vk_arr)
            .map_err(|_| (StatusCode::BAD_REQUEST, "Bad pubkey".into()))?;

        // Signed message
        vk.verify(ts.as_bytes(), &sig)
            .map_err(|_| (StatusCode::UNAUTHORIZED, "Signature mismatch".into()))?;

        // Fetch Device based on signature
        let device = device_repository::get_device_by_identity_key(&state.pool, ik_b64)
            .await
            .map_err(|_| (StatusCode::UNAUTHORIZED, "Unknown device".into()))?;

        Ok(AuthenticatedDevice(device))
    }
}
