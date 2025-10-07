use sqlx::{PgPool, Result};
use uuid::Uuid;
use chrono::NaiveDateTime;
use crate::models::session::PendingSession;

pub async fn create_session(
    pool: &PgPool,
    sender_device_id: &Uuid,
    recipient_device_id: &Uuid,
    ephemeral_pubkey: &str,
    ciphertext: &str,
) -> Result<PendingSession, sqlx::Error> {
    let session = sqlx::query_as!(
        PendingSession,
        r#"
        INSERT INTO pending_sessions (
            sender_device_id,
            recipient_device_id,
            ephemeral_pubkey,
            ciphertext
        )
        VALUES ($1, $2, $3, $4)
        RETURNING id, sender_device_id, recipient_device_id, ephemeral_pubkey, ciphertext, created_at
        "#,
        sender_device_id,
        recipient_device_id,
        ephemeral_pubkey,
        ciphertext
    )
    .fetch_one(pool)
    .await?;

    Ok(session)
}
