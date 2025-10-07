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

pub async fn get_devices_by_user_id(
    pool: &PgPool,
    user_id: &Uuid,
) -> Result<Vec<Devices>, sqlx::Error> {
    let devices = sqlx::query_as!(
        Devices,
        r#"
        SELECT
            id,
            user_id,
            identity_pubkey,
            signed_prekey_pub,
            signed_prekey_sig,
            one_time_prekeys,
            device_label,
            push_token,
            last_seen,
            created_at
        FROM devices
        WHERE user_id = $1
        ORDER BY created_at DESC
        "#,
        user_id
    )
    .fetch_all(pool)
    .await?;

    Ok(devices)
}

pub async fn get_device_by_identity_key(
    pool: &PgPool,
    id_key: &str,
) -> Result<Devices, sqlx::Error> {
    let devices = sqlx::query_as!(
        Devices,
        r#"
        SELECT
            id,
            user_id,
            identity_pubkey,
            signed_prekey_pub,
            signed_prekey_sig,
            one_time_prekeys,
            device_label,
            push_token,
            last_seen,
            created_at
        FROM devices
        WHERE identity_pubkey = $1
        ORDER BY created_at DESC
        "#,
        id_key
    )
    .fetch_one(pool)
    .await?;

    Ok(devices)
}