use crate::models::enrollment_token::EnrollmentClaims;
use chrono::{Duration, Utc};
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use uuid::Uuid;

pub fn generate_enrollment_tokens(user_id: &Uuid, secret: &str) -> String {
    let claims = EnrollmentClaims {
        sub: user_id.to_string(),
        exp: (Utc::now() + Duration::minutes(5)).timestamp() as usize,
    };
    return encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    )
    .expect("Failed to generate enrollment token");
}

pub fn verify_enrollment_token(token: &str, secret: &str) -> Option<Uuid> {
    let data = decode::<EnrollmentClaims>(
        token,
        &DecodingKey::from_secret(secret.as_bytes()),
        &Validation::default(),
    ).ok()?;
    Uuid::parse_str(&data.claims.sub).ok()
}
