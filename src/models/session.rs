use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize)]
pub struct PendingSession {
    pub id: Uuid,
    pub sender_device_id: Uuid,
    pub recipient_device_id: Uuid,
    pub ephemeral_pubkey: String,
    pub sender_prekey_pub: String,
    pub otpk_used: String,
    pub ciphertext: String,
    pub created_at: Option<NaiveDateTime>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub id: Uuid,
    pub chat_id: Option<Uuid>,
    pub sender_device_id: Uuid,
    pub receiver_device_id: Uuid,
    pub root_key: Vec<u8>,
    pub send_chain_key: Option<Vec<u8>>,
    pub recv_chain_key: Option<Vec<u8>>,
    pub send_counter: i32,
    pub recv_counter: i32,
    pub ratchet_pub: Option<Vec<u8>>,
    pub last_remote_pub: Option<Vec<u8>>,
    pub created_at: Option<NaiveDateTime>,
    pub updated_at: Option<NaiveDateTime>,
}