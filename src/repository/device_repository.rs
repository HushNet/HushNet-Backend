use std::borrow::Cow;

use crate::models::{device::{DeviceBundle, Devices}, user::User};
use sqlx::{PgPool, Result};
use uuid::Uuid;

pub async fn create_device(
    pool: &PgPool,
    user_id: &Uuid,
    identity_pubkey: &str,
    prekey_pubkey: &str,
    signed_prekey_pub: &str,
    signed_prekey_sig: &str,
    one_time_prekeys: &serde_json::Value,
    device_label: &str,
    push_token: &str,
) -> Result<Devices> {
    let device = sqlx::query_as!(
        Devices,
        r#"
        INSERT INTO devices (user_id, identity_pubkey, prekey_pubkey, signed_prekey_pub, signed_prekey_sig, one_time_prekeys, device_label, push_token)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
        RETURNING id, user_id, identity_pubkey, prekey_pubkey, signed_prekey_pub, signed_prekey_sig, one_time_prekeys, device_label, push_token, last_seen, created_at
        "#,
        user_id,
        identity_pubkey,
        prekey_pubkey,
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
            prekey_pubkey,
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
            prekey_pubkey,
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


pub async fn get_device_bundle(
    pool: &PgPool,
    user_id: &Uuid,
) -> Result<Vec<DeviceBundle>, sqlx::Error> {
    let rows = sqlx::query!(
        r#"
        SELECT id, identity_pubkey, prekey_pubkey, signed_prekey_pub, signed_prekey_sig, one_time_prekeys
        FROM devices
        WHERE user_id = $1
        "#,
        user_id
    )
    .fetch_all(pool)
    .await?;

    let mut bundles = Vec::new();
    for row in rows {
        let otpks: Vec<String> = serde_json::from_value(row.one_time_prekeys).map_err(|e| {
            sqlx::Error::ColumnDecode {
                index: Cow::from("one_time_prekeys").to_string(),
                source: Box::new(e), // serde_json::Error impl Error + Send + Sync
            }
        })?;

        bundles.push(DeviceBundle {
            device_id: row.id,
            identity_pubkey: row.identity_pubkey,
            signed_prekey_pub: row.signed_prekey_pub,
            signed_prekey_sig: row.signed_prekey_sig,
            one_time_prekeys: otpks,
        });
    }

    Ok(bundles)
}

pub async fn get_user_for_device(
    pool: &PgPool,
    device_id: &Uuid,
) -> Result<Option<User>, sqlx::Error> {
    let row = sqlx::query!(
        r#"
        SELECT user_id
        FROM devices
        WHERE id = $1
        "#,
        device_id
    )
    .fetch_one(pool)
    .await?;

    let user_data = sqlx::query_as!(
        User,
        r#"
        SELECT
            id,
            username,
            created_at
        FROM users
        WHERE id = $1
        "#,
        row.user_id
    )
    .fetch_optional(pool)
    .await?;

    Ok(user_data)
}