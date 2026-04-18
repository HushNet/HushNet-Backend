use sqlx::PgPool;
use uuid::Uuid;

use crate::models::federation::{FederationNode, FederationOutboxEntry};

// ─── federation_nodes ────────────────────────────────────────────────────────

/// Insert or update a peer node record. Called after a successful registry
/// lookup; the last_seen timestamp is refreshed on every upsert.
pub async fn upsert_federation_node(
    pool: &PgPool,
    node_id: &str,
    api_url: &str,
    public_key_b64: &str,
) -> Result<FederationNode, sqlx::Error> {
    sqlx::query_as!(
        FederationNode,
        r#"
        INSERT INTO federation_nodes (node_id, api_url, public_key_b64)
        VALUES ($1, $2, $3)
        ON CONFLICT (node_id) DO UPDATE
          SET api_url        = EXCLUDED.api_url,
              public_key_b64 = EXCLUDED.public_key_b64,
              last_seen      = NOW()
        RETURNING id, node_id, api_url, public_key_b64, last_seen, is_blocked, created_at
        "#,
        node_id,
        api_url,
        public_key_b64,
    )
    .fetch_one(pool)
    .await
}

pub async fn get_federation_node(
    pool: &PgPool,
    node_id: &str,
) -> Result<Option<FederationNode>, sqlx::Error> {
    sqlx::query_as!(
        FederationNode,
        r#"
        SELECT id, node_id, api_url, public_key_b64, last_seen, is_blocked, created_at
        FROM federation_nodes
        WHERE node_id = $1
        "#,
        node_id,
    )
    .fetch_optional(pool)
    .await
}

// ─── used_node_nonces ────────────────────────────────────────────────────────

/// Try to claim a (nonce, node_id) pair atomically.
///
/// Returns true if the nonce was fresh (insert succeeded), false if it already
/// existed (replay detected). The INSERT ... ON CONFLICT DO NOTHING pattern is
/// safe under concurrent requests: at most one INSERT per (nonce, node_id) pair
/// can succeed within a single PostgreSQL transaction.
pub async fn claim_nonce(
    pool: &PgPool,
    node_id: &str,
    nonce: &str,
) -> Result<bool, sqlx::Error> {
    let result = sqlx::query!(
        r#"
        INSERT INTO used_node_nonces (nonce, node_id)
        VALUES ($1, $2)
        ON CONFLICT DO NOTHING
        "#,
        nonce,
        node_id,
    )
    .execute(pool)
    .await?;
    Ok(result.rows_affected() == 1)
}

/// Delete nonces older than 5 minutes. The acceptance window is 60 s, so any
/// nonce older than 5 minutes is guaranteed to be outside that window and will
/// never be re-accepted even if deleted.
pub async fn purge_expired_nonces(pool: &PgPool) -> Result<u64, sqlx::Error> {
    let result = sqlx::query!(
        "DELETE FROM used_node_nonces WHERE used_at < NOW() - INTERVAL '5 minutes'"
    )
    .execute(pool)
    .await?;
    Ok(result.rows_affected())
}

// ─── federation_outbox ───────────────────────────────────────────────────────

/// Enqueue a new outbound message for delivery to target_node_id.
/// Returns the UUID of the created outbox entry.
pub async fn enqueue_outbox(
    pool: &PgPool,
    target_node_id: &str,
    logical_msg_id: &str,
    payload: &serde_json::Value,
) -> Result<Uuid, sqlx::Error> {
    let id = sqlx::query_scalar!(
        r#"
        INSERT INTO federation_outbox (target_node_id, logical_msg_id, payload)
        VALUES ($1, $2, $3)
        RETURNING id
        "#,
        target_node_id,
        logical_msg_id,
        payload,
    )
    .fetch_one(pool)
    .await?;
    Ok(id)
}

/// Fetch all pending entries whose next_attempt is now or past.
/// Capped at 100 per poll cycle to bound per-iteration latency.
pub async fn fetch_due_outbox_entries(
    pool: &PgPool,
) -> Result<Vec<FederationOutboxEntry>, sqlx::Error> {
    sqlx::query_as!(
        FederationOutboxEntry,
        r#"
        SELECT id, target_node_id, logical_msg_id, payload,
               attempt_count, last_attempt, next_attempt, status, created_at
        FROM federation_outbox
        WHERE status = 'pending'
          AND next_attempt <= NOW()
        ORDER BY next_attempt ASC
        LIMIT 100
        "#,
    )
    .fetch_all(pool)
    .await
}

pub async fn mark_outbox_delivered(pool: &PgPool, id: Uuid) -> Result<(), sqlx::Error> {
    sqlx::query!(
        "UPDATE federation_outbox SET status = 'delivered', last_attempt = NOW() WHERE id = $1",
        id,
    )
    .execute(pool)
    .await?;
    Ok(())
}

/// Mark all pending entries for a logical_msg_id as delivered.
/// Called when Node B sends a POST /s2s/ack back to Node A.
pub async fn mark_outbox_delivered_by_logical_id(
    pool: &PgPool,
    logical_msg_id: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query!(
        "UPDATE federation_outbox SET status = 'delivered', last_attempt = NOW()
         WHERE logical_msg_id = $1 AND status = 'pending'",
        logical_msg_id,
    )
    .execute(pool)
    .await?;
    Ok(())
}

/// Advance the retry schedule for a failed delivery attempt.
///
/// Backoff in seconds: 10 * 2^attempt_count, capped at 3600 (one hour).
/// After max_attempts the entry is permanently marked 'failed'.
pub async fn record_outbox_failure(
    pool: &PgPool,
    id: Uuid,
    attempt_count: i32,
    max_attempts: i32,
) -> Result<(), sqlx::Error> {
    if attempt_count >= max_attempts {
        sqlx::query!(
            "UPDATE federation_outbox
             SET status = 'failed', last_attempt = NOW(), attempt_count = $2
             WHERE id = $1",
            id,
            attempt_count,
        )
        .execute(pool)
        .await?;
    } else {
        let backoff_secs = (10_i64 * (1_i64 << attempt_count.min(12))).min(3600);
        sqlx::query!(
            r#"
            UPDATE federation_outbox
            SET attempt_count = $2,
                last_attempt  = NOW(),
                next_attempt  = NOW() + ($3 || ' seconds')::interval
            WHERE id = $1
            "#,
            id,
            attempt_count,
            backoff_secs.to_string(),
        )
        .execute(pool)
        .await?;
    }
    Ok(())
}

// ─── Shadow user / device creation ───────────────────────────────────────────
//
// When this node (Node B) receives a message from a remote user (Alice on
// Node A), it needs valid rows in users and devices to satisfy the FK
// constraints on messages.from_user_id and messages.from_device_id.
//
// Shadow rows are identified by a non-NULL home_node_id. They are never
// returned by normal user lookup endpoints, and their devices are never
// queried for prekey material.

/// Upsert a shadow user record for a remote user.
/// Conflict key is federated_address (globally unique).
/// Returns the local UUID of the (possibly newly created) shadow user.
pub async fn upsert_shadow_user(
    pool: &PgPool,
    username: &str,
    federated_address: &str,
    home_node_id: Uuid,
) -> Result<Uuid, sqlx::Error> {
    let id = sqlx::query_scalar!(
        r#"
        INSERT INTO users (username, federated_address, home_node_id)
        VALUES ($1, $2, $3)
        ON CONFLICT (federated_address) DO UPDATE
          SET username = EXCLUDED.username
        RETURNING id
        "#,
        username,
        federated_address,
        home_node_id,
    )
    .fetch_one(pool)
    .await?;
    Ok(id)
}

/// Upsert a shadow device record for a remote device.
///
/// The device UUID is reused from Node A (random UUID collision probability
/// is negligible across nodes). Prekey fields are stored as empty values
/// because shadow devices are never queried for key material; they exist
/// solely to satisfy FK constraints.
pub async fn upsert_shadow_device(
    pool: &PgPool,
    device_id: Uuid,
    user_id: Uuid,
    identity_pubkey: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query!(
        r#"
        INSERT INTO devices (
            id, user_id, identity_pubkey,
            prekey_pubkey, signed_prekey_pub, signed_prekey_sig, one_time_prekeys
        )
        VALUES ($1, $2, $3, '', '', '', '[]'::jsonb)
        ON CONFLICT (id) DO NOTHING
        "#,
        device_id,
        user_id,
        identity_pubkey,
    )
    .execute(pool)
    .await?;
    Ok(())
}

/// Find or create a direct chat between two users.
///
/// Enforces the schema constraint user_a < user_b by sorting before insert.
/// The unique index on (LEAST(user_a,user_b), GREATEST(user_a,user_b)) prevents
/// duplicate chats regardless of the argument order.
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

    if let Some(id) = sqlx::query_scalar!(
        "SELECT id FROM chats WHERE user_a = $1 AND user_b = $2 AND chat_type = 'direct'",
        ua,
        ub,
    )
    .fetch_optional(pool)
    .await?
    {
        return Ok(id);
    }

    let id = sqlx::query_scalar!(
        r#"
        INSERT INTO chats (user_a, user_b, chat_type)
        VALUES ($1, $2, 'direct')
        RETURNING id
        "#,
        ua,
        ub,
    )
    .fetch_one(pool)
    .await?;

    Ok(id)
}

/// Look up a local user (home_node_id IS NULL) by username.
/// Returns None if the user does not exist or is a shadow record.
pub async fn get_local_user_id_by_username(
    pool: &PgPool,
    username: &str,
) -> Result<Option<Uuid>, sqlx::Error> {
    let row = sqlx::query!(
        "SELECT id FROM users WHERE username = $1 AND home_node_id IS NULL",
        username,
    )
    .fetch_optional(pool)
    .await?;
    Ok(row.map(|r| r.id))
}
