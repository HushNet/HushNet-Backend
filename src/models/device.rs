use chrono::NaiveDateTime;
use serde::{Serialize, Deserialize};
use serde_json::Value;
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize)]
pub struct Devices {
    pub id: Uuid,
    pub user_id: Uuid,
    pub identity_pubkey: String,
    pub signed_prekey_pub: String,
    pub signed_prekey_sig: String,
    pub one_time_prekeys: Value,
    pub device_label: Option<String>,
    pub push_token: Option<String>,
    pub last_seen: Option<NaiveDateTime>,
    pub created_at: Option<NaiveDateTime>,
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