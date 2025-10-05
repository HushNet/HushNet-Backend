use serde::{Serialize, Deserialize};
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize)]
pub struct Devices {
    pub id: Uuid,
    pub user_id: Uuid,
    pub identity_pubkey: String,
    pub device_label: Option<String>,    // <- nullable in DB => Option
    pub push_token: Option<String>,      // <- nullable in DB => Option
    pub last_seen: Option<chrono::NaiveDateTime>,
    pub created_at: Option<chrono::NaiveDateTime>
}