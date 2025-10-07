use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize)]
pub struct PendingSession {
    pub id: Uuid,
    pub sender_device_id: Uuid,
    pub recipient_device_id: Uuid,
    pub ephemeral_pubkey: String,
    pub ciphertext: String,
    pub created_at: Option<NaiveDateTime>
}