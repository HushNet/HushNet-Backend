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
    pub identity_pubkey: String,
    pub signed_prekey: SignedPreKey,
    pub one_time_prekeys: Vec<OneTimePrekeys>,
    pub device_label: String,
    pub push_token: String,
    pub last_seen: DateTime<Utc>,
    pub created_at: DateTime<Utc>
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SignedPreKey {
    pub key: String,
    pub signature: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct OneTimePrekeys {
    pub key: String
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UsedToken {
    pub token_value: String,
    pub used_at: DateTime<Utc>
}
