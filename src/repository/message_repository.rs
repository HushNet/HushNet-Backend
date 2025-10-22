use axum::{extract::State, Json};
use sqlx::PgPool;
use crate::{middlewares::auth::AuthenticatedDevice, models::message::{Message, MessageView, OutgoingMessage}};
use uuid::Uuid;
use serde_json::Value;
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
