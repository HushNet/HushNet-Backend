use crate::models::device::Devices;
use sqlx::{PgPool, Result};
use uuid::Uuid;

pub async fn create_device(
    pool: &PgPool,
    user_id: &Uuid,
    identity_pubkey: &str,
    signed_prekey_pub: &str,
    signed_prekey_sig: &str,
    one_time_prekeys: &serde_json::Value,
    device_label: &str,
    push_token: &str,
) -> Result<Devices> {
    let device = sqlx::query_as!(
        Devices,
        r#"
        INSERT INTO devices (user_id, identity_pubkey, signed_prekey_pub, signed_prekey_sig, one_time_prekeys, device_label, push_token)
        VALUES ($1, $2, $3, $4, $5, $6, $7)
        RETURNING id, user_id, identity_pubkey, signed_prekey_pub, signed_prekey_sig, one_time_prekeys, device_label, push_token, last_seen, created_at
        "#,
        user_id,
        identity_pubkey,
        signed_prekey_pub,
        signed_prekey_sig,
        one_time_prekeys,
        device_label,
        push_token
    )
    .fetch_one(pool)
    .await?;

    Ok(device)
}
