use crate::{
    middlewares::auth::AuthenticatedDevice,
    models::message::{MessageView, OutgoingMessage},
};
use serde_json::Value;
use sqlx::PgPool;
use uuid::Uuid;

pub async fn insert_message(
    pool: &PgPool,
    from_device_id: Uuid,
    from_user_id: Uuid,
    msg: OutgoingMessage,
) -> Result<(), sqlx::Error> {
    for payload in msg.payloads {
        sqlx::query!(
            r#"
            INSERT INTO messages (
                logical_msg_id,
                chat_id,
                from_user_id,
                from_device_id,
                to_user_id,
                to_device_id,
                header,
                ciphertext
            )
            VALUES ($1,$2,$3,$4,$5,$6,$7,$8)
            "#,
            msg.logical_msg_id,
            msg.chat_id,
            from_user_id,
            from_device_id,
            msg.to_user_id,
            payload.to_device_id,
            payload.header,
            payload.ciphertext
        )
        .execute(pool)
        .await?;
    }

    Ok(())
}

/// Insert a message that arrived via S2S forwarding from a peer node.
///
/// Idempotent: if a row with the same (logical_msg_id, to_device_id) already
/// exists (duplicate delivery from outbox retry), the INSERT is skipped and
/// the function returns Ok(false). Returns Ok(true) when a new row is created.
///
/// The unique constraint `uniq_message_per_device` (added in federation.sql)
/// makes the ON CONFLICT clause safe without a preceding SELECT.
pub async fn insert_federated_message(
    pool: &PgPool,
    logical_msg_id: &str,
    chat_id: Uuid,
    from_user_id: Uuid,
    from_device_id: Uuid,
    to_user_id: Uuid,
    to_device_id: Uuid,
    header: &Value,
    ciphertext: &str,
) -> Result<bool, sqlx::Error> {
    let result = sqlx::query(
        "INSERT INTO messages (
             logical_msg_id, chat_id,
             from_user_id, from_device_id,
             to_user_id, to_device_id,
             header, ciphertext
         )
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
         ON CONFLICT (logical_msg_id, to_device_id) DO NOTHING",
    )
    .bind(logical_msg_id)
    .bind(chat_id)
    .bind(from_user_id)
    .bind(from_device_id)
    .bind(to_user_id)
    .bind(to_device_id)
    .bind(header)
    .bind(ciphertext)
    .execute(pool)
    .await?;
    Ok(result.rows_affected() == 1)
}

pub async fn fetch_pending_messages(
    pool: &PgPool,
    AuthenticatedDevice(device): AuthenticatedDevice,
) -> Result<Vec<MessageView>, sqlx::Error> {
    let messages = sqlx::query_as!(
        MessageView,
        r#"
            SELECT 
                id,
                logical_msg_id,
                chat_id,
                from_user_id,
                from_device_id,
                header as "header: Value",
                ciphertext,
                created_at as "created_at?"
            FROM messages
            WHERE to_device_id = $1
            AND delivered_at IS NULL
            ORDER BY created_at ASC
        "#,
        device.id
    )
    .fetch_all(pool)
    .await?;

    // Mark as delivered
    sqlx::query!(
        "UPDATE messages SET delivered_at = NOW() WHERE to_device_id = $1 AND delivered_at IS NULL",
        device.id
    )
    .execute(pool)
    .await?;

    Ok(messages)
}
