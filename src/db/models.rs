use serde::{Serialize, Deserialize};
use chrono::{DateTime, Utc};
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize)]
pub struct User {
    pub id: Uuid,
    pub username: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Message {
    pub id: Uuid,
    pub from_user: Uuid,
    pub to_user: Uuid,
    pub ciphertext: String,
    pub created_at: DateTime<Utc>,
    pub delivered: bool
}
#[derive(Debug, Serialize, Deserialize)]
pub struct Devices {
    pub id: Uuid,
    pub user_id: Uuid,
    pub identity_pubkeyy: String,
    pub device_label: String,
    pub push_token: String,
    pub last_seen: DateTime<Utc>,
    pub created_at: DateTime<Utc>
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SignedPrekeys {
    pub id: Uuid,
    pub device_id: Uuid,
    pub key: String, // SPK Key
    pub signature: String, // Sig (SPK, IK, prv)
    pub created_at: DateTime<Utc>
}

#[derive(Debug, Serialize, Deserialize)]
pub struct OneTimePrekeys {
    pub id: Uuid,
    pub device_id: Uuid,
    pub key: String,
    pub used: bool,
    pub created_at: DateTime<Utc>
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UsedToken {
    pub token_value: String,
    pub used_at: DateTime<Utc>
}
