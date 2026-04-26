use sqlx::PgPool;
use uuid::Uuid;

use crate::models::federation::{FederationNode, FederationOutboxEntry};

// Non-macro sqlx throughout: avoids compile-time DATABASE_URL requirement and
// the need to run `cargo sqlx prepare` every time a query changes.

// ─── federation_nodes ────────────────────────────────────────────────────────

pub async fn upsert_federation_node(
    pool: &PgPool,
    node_id: &str,
    api_url: &str,
    public_key_b64: &str,
) -> Result<FederationNode, sqlx::Error> {
    sqlx::query_as::<_, FederationNode>(
        r#"
        INSERT INTO federation_nodes (node_id, api_url, public_key_b64)
        VALUES ($1, $2, $3)
        ON CONFLICT (node_id) DO UPDATE
          SET api_url        = EXCLUDED.api_url,
              public_key_b64 = EXCLUDED.public_key_b64,
              last_seen      = NOW()
        RETURNING id, node_id, api_url, public_key_b64, last_seen, is_blocked, created_at
        "#,
    )
    .bind(node_id)
    .bind(api_url)
    .bind(public_key_b64)
    .fetch_one(pool)
    .await
}

pub async fn get_federation_node(
    pool: &PgPool,
    node_id: &str,
) -> Result<Option<FederationNode>, sqlx::Error> {
    sqlx::query_as::<_, FederationNode>(
        "SELECT id, node_id, api_url, public_key_b64, last_seen, is_blocked, created_at
         FROM federation_nodes WHERE node_id = $1",
    )
    .bind(node_id)
    .fetch_optional(pool)
    .await
}

// ─── used_node_nonces ────────────────────────────────────────────────────────

/// Returns true if the nonce was fresh (not seen before), false on replay.
pub async fn claim_nonce(pool: &PgPool, node_id: &str, nonce: &str) -> Result<bool, sqlx::Error> {
    let result = sqlx::query(
        "INSERT INTO used_node_nonces (nonce, node_id) VALUES ($1, $2) ON CONFLICT DO NOTHING",
    )
    .bind(nonce)
    .bind(node_id)
    .execute(pool)
    .await?;
    Ok(result.rows_affected() == 1)
}

pub async fn purge_expired_nonces(pool: &PgPool) -> Result<u64, sqlx::Error> {
    let result =
        sqlx::query("DELETE FROM used_node_nonces WHERE used_at < NOW() - INTERVAL '5 minutes'")
            .execute(pool)
            .await?;
    Ok(result.rows_affected())
}

// ─── federation_outbox ───────────────────────────────────────────────────────

pub async fn enqueue_outbox(
    pool: &PgPool,
    target_node_id: &str,
    logical_msg_id: &str,
    payload: &serde_json::Value,
) -> Result<Uuid, sqlx::Error> {
    let row: (Uuid,) = sqlx::query_as(
        "INSERT INTO federation_outbox (target_node_id, logical_msg_id, payload)
         VALUES ($1, $2, $3) RETURNING id",
    )
    .bind(target_node_id)
    .bind(logical_msg_id)
    .bind(payload)
    .fetch_one(pool)
    .await?;
    Ok(row.0)
}

pub async fn fetch_due_outbox_entries(
    pool: &PgPool,
) -> Result<Vec<FederationOutboxEntry>, sqlx::Error> {
    sqlx::query_as::<_, FederationOutboxEntry>(
        "SELECT id, target_node_id, logical_msg_id, payload,
                attempt_count, last_attempt, next_attempt, status, created_at
         FROM federation_outbox
         WHERE status = 'pending' AND next_attempt <= NOW()
         ORDER BY next_attempt ASC LIMIT 100",
    )
    .fetch_all(pool)
    .await
}

pub async fn mark_outbox_delivered(pool: &PgPool, id: Uuid) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE federation_outbox SET status = 'delivered', last_attempt = NOW() WHERE id = $1",
    )
    .bind(id)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn mark_outbox_delivered_by_logical_id(
    pool: &PgPool,
    logical_msg_id: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE federation_outbox SET status = 'delivered', last_attempt = NOW()
         WHERE logical_msg_id = $1 AND status = 'pending'",
    )
    .bind(logical_msg_id)
    .execute(pool)
    .await?;
    Ok(())
}

/// Exponential backoff: 10s * 2^attempt, capped at 3600s.
/// Marks 'failed' after max_attempts.
pub async fn record_outbox_failure(
    pool: &PgPool,
    id: Uuid,
    attempt_count: i32,
    max_attempts: i32,
) -> Result<(), sqlx::Error> {
    if attempt_count >= max_attempts {
        sqlx::query(
            "UPDATE federation_outbox
             SET status = 'failed', last_attempt = NOW(), attempt_count = $2
             WHERE id = $1",
        )
        .bind(id)
        .bind(attempt_count)
        .execute(pool)
        .await?;
    } else {
        let backoff_secs = (10_i64 * (1_i64 << attempt_count.min(12))).min(3600);
        sqlx::query(
            "UPDATE federation_outbox
             SET attempt_count = $2,
                 last_attempt  = NOW(),
                 next_attempt  = NOW() + ($3 || ' seconds')::interval
             WHERE id = $1",
        )
        .bind(id)
        .bind(attempt_count)
        .bind(backoff_secs.to_string())
        .execute(pool)
        .await?;
    }
    Ok(())
}

// ─── Shadow user / device ────────────────────────────────────────────────────

pub async fn upsert_shadow_user(
    pool: &PgPool,
    username: &str,
    federated_address: &str,
    home_node_id: Uuid,
) -> Result<Uuid, sqlx::Error> {
    let row: (Uuid,) = sqlx::query_as(
        "INSERT INTO users (username, federated_address, home_node_id)
         VALUES ($1, $2, $3)
         ON CONFLICT (federated_address) DO UPDATE SET username = EXCLUDED.username
         RETURNING id",
    )
    .bind(username)
    .bind(federated_address)
    .bind(home_node_id)
    .fetch_one(pool)
    .await?;
    Ok(row.0)
}

pub async fn upsert_shadow_device(
    pool: &PgPool,
    device_id: Uuid,
    user_id: Uuid,
    identity_pubkey: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO devices (id, user_id, identity_pubkey,
             prekey_pubkey, signed_prekey_pub, signed_prekey_sig, one_time_prekeys)
         VALUES ($1, $2, $3, '', '', '', '[]'::jsonb)
         ON CONFLICT (id) DO NOTHING",
    )
    .bind(device_id)
    .bind(user_id)
    .bind(identity_pubkey)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn get_or_create_direct_chat(
    pool: &PgPool,
    user_x: Uuid,
    user_y: Uuid,
) -> Result<Uuid, sqlx::Error> {
    let (ua, ub) = if user_x < user_y {
        (user_x, user_y)
    } else {
        (user_y, user_x)
    };

    if let Some(row) = sqlx::query_as::<_, (Uuid,)>(
        "SELECT id FROM chats WHERE user_a = $1 AND user_b = $2 AND chat_type = 'direct'",
    )
    .bind(ua)
    .bind(ub)
    .fetch_optional(pool)
    .await?
    {
        return Ok(row.0);
    }

    let row: (Uuid,) = sqlx::query_as(
        "INSERT INTO chats (user_a, user_b, chat_type) VALUES ($1, $2, 'direct') RETURNING id",
    )
    .bind(ua)
    .bind(ub)
    .fetch_one(pool)
    .await?;
    Ok(row.0)
}

/// Returns the UUID of a local (non-shadow) user by username.
/// Returns None if the user does not exist or is a shadow record.
pub async fn get_local_user_id_by_username(
    pool: &PgPool,
    username: &str,
) -> Result<Option<Uuid>, sqlx::Error> {
    let row = sqlx::query_as::<_, (Uuid,)>(
        "SELECT id FROM users WHERE username = $1 AND home_node_id IS NULL",
    )
    .bind(username)
    .fetch_optional(pool)
    .await?;
    Ok(row.map(|r| r.0))
}
