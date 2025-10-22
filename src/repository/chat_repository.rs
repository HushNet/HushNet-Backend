use sqlx::{PgPool, Result};
use uuid::Uuid;

use crate::{middlewares::auth::AuthenticatedDevice, models::{chat::ChatView}};

pub async fn get_chats_for_device(
    pool: &PgPool,
    AuthenticatedDevice(device): AuthenticatedDevice
) -> Result<Vec<ChatView>, sqlx::Error> {
    // Get current user_id from the device
    let user_id: Option<Uuid> = sqlx::query_scalar!(
        r#"SELECT user_id FROM devices WHERE id = $1"#,
        device.id
    )
    .fetch_optional(pool)
    .await?;

    let user_id = match user_id {
        Some(id) => id,
        None => return Ok(vec![]),
    };

    // Fetch all chats + partner info
    let chats = sqlx::query_as!(
        ChatView,
        r#"
        SELECT 
            c.id,
            c.chat_type,
            CASE 
                WHEN c.user_a = $1 THEN c.user_b
                ELSE c.user_a
            END AS partner_user_id,
            (
                SELECT u.username 
                FROM users u
                WHERE u.id = CASE 
                    WHEN c.user_a = $1 THEN c.user_b
                    ELSE c.user_a
                END
            ) AS partner_username,
            c.name,
            c.last_message_id,
            c.updated_at
        FROM chats c
        WHERE 
            (c.chat_type = 'direct' AND ($1 IN (c.user_a, c.user_b)))
            OR (c.chat_type = 'group' AND c.id IN (
                SELECT chat_id FROM chat_members WHERE user_id = $1
            ))
        ORDER BY c.updated_at DESC
        "#,
        user_id
    )
    .fetch_all(pool)
    .await?;

    Ok(chats)
}

