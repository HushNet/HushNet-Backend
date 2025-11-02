use sqlx::{PgPool, Result};
use uuid::Uuid;

use crate::{middlewares::auth::AuthenticatedDevice, models::session::PendingSession};

pub async fn create_pending_session(
    pool: &PgPool,
    sender_device_id: &Uuid,
    recipient_device_id: &Uuid,
    ephemeral_pubkey: &str,
    sender_prekey_pub: &str,
    otpk_used: &str,
    ciphertext: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query_as!(
        PendingSession,
        r#"
        INSERT INTO pending_sessions (
            sender_device_id,
            recipient_device_id,
            ephemeral_pubkey,
            sender_prekey_pub,
            otpk_used,
            ciphertext
        )
        VALUES ($1, $2, $3, $4, $5, $6)
        ON CONFLICT DO NOTHING
        "#,
        sender_device_id,
        recipient_device_id,
        ephemeral_pubkey,
        sender_prekey_pub,
        otpk_used,
        ciphertext
    )
    .execute(pool)
    .await?;

    Ok(())
}

pub async fn get_pending_sessions(
    pool: &PgPool,
    AuthenticatedDevice(device): AuthenticatedDevice,
) -> Result<Vec<PendingSession>, sqlx::Error> {
    let sessions = sqlx::query_as!(
        PendingSession,
        r#"
        SELECT 
            id,
            sender_device_id,
            ephemeral_pubkey,
            ciphertext,
            recipient_device_id,
            sender_prekey_pub,
            otpk_used,
            created_at
        FROM pending_sessions
        WHERE recipient_device_id = $1
        "#,
        device.id
    )
    .fetch_all(pool)
    .await?;

    Ok(sessions)
}

pub async fn get_pending_session_by_id(
    pool: &PgPool,
    pending_id: &Uuid,
    recipient_device_id: &Uuid,
) -> Result<Option<PendingSession>, sqlx::Error> {
    let session = sqlx::query_as!(
        PendingSession,
        r#"
        SELECT 
            id,
            sender_device_id,
            ephemeral_pubkey,
            ciphertext,
            recipient_device_id,
            sender_prekey_pub,
            otpk_used,
            created_at
        FROM pending_sessions
        WHERE id = $1 AND recipient_device_id = $2
        "#,
        pending_id,
        recipient_device_id
    )
    .fetch_optional(pool)
    .await?;

    Ok(session)
}

pub async fn get_or_create_chat_id(
    pool: &PgPool,
    sender_device_id: &Uuid,
    receiver_device_id: &Uuid,
) -> Result<Uuid, sqlx::Error> {
    // essaie d’abord de retrouver un chat existant (dans les deux sens)
    if let Some(chat_id) = sqlx::query_scalar!(
        r#"
        SELECT id FROM chats
        WHERE (user_a= LEAST(
                    (SELECT user_id FROM devices WHERE id=$1),
                    (SELECT user_id FROM devices WHERE id=$2)
               )
           AND user_b = GREATEST(
                    (SELECT user_id FROM devices WHERE id=$1),
                    (SELECT user_id FROM devices WHERE id=$2)
               ))
        "#,
        sender_device_id,
        receiver_device_id
    )
    .fetch_optional(pool)
    .await?
    {
        return Ok(chat_id);
    }

    // sinon crée-le proprement en respectant la contrainte
    let new_chat_id = sqlx::query_scalar!(
        r#"
        INSERT INTO chats (user_a, user_b)
        VALUES (
            LEAST(
              (SELECT user_id FROM devices WHERE id=$1),
              (SELECT user_id FROM devices WHERE id=$2)
            ),
            GREATEST(
              (SELECT user_id FROM devices WHERE id=$1),
              (SELECT user_id FROM devices WHERE id=$2)
            )
        )
        RETURNING id
        "#,
        sender_device_id,
        receiver_device_id
    )
    .fetch_one(pool)
    .await?;

    Ok(new_chat_id)
}

pub async fn insert_or_update_session(
    pool: &PgPool,
    chat_id: &Uuid,
    sender_device_id: &Uuid,
    receiver_device_id: &Uuid,
) -> Result<(), sqlx::Error> {
    sqlx::query!(
        r#"
        INSERT INTO sessions (
          chat_id, sender_device_id, receiver_device_id
        )
        VALUES ($1, $2, $3)
        ON CONFLICT (sender_device_id, receiver_device_id)
        DO UPDATE SET
          updated_at = NOW()
        "#,
        chat_id,
        sender_device_id,
        receiver_device_id,
    )
    .execute(pool)
    .await?;

    Ok(())
}

pub async fn delete_pending_session(pool: &PgPool, pending_id: &Uuid) -> Result<(), sqlx::Error> {
    sqlx::query!("DELETE FROM pending_sessions WHERE id = $1", pending_id)
        .execute(pool)
        .await?;

    Ok(())
}
