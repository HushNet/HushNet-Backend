use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;
use chrono::NaiveDateTime;

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow, Clone)]
pub struct Message {
    pub id: Uuid,
    pub logical_msg_id: String,
    pub chat_id: Uuid,
    pub from_user_id: Uuid,
    pub from_device_id: Uuid,
    pub to_user_id: Uuid,
    pub to_device_id: Uuid,
    pub header: Value,            // Double Ratchet header (JSON)
    pub ciphertext: String,           // base64(nonce || cipher || mac)
    pub delivered_at: Option<NaiveDateTime>,
    pub read_at: Option<NaiveDateTime>,
    pub created_at: NaiveDateTime,
}


/// Each encrypted payload is specific to a recipient device.
#[derive(Debug, Deserialize)]
pub struct OutgoingMessagePayload {
    pub to_device_id: Uuid,
    pub header: Value,
    pub ciphertext: String,
}

/// Represents the logical message (fan-out over multiple recipient devices).
#[derive(Debug, Deserialize)]
pub struct OutgoingMessage {
    pub chat_id: Uuid,
    pub logical_msg_id: String,
    pub to_user_id: Uuid,
    pub payloads: Vec<OutgoingMessagePayload>,
}



#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct MessageView {
    pub id: Uuid,
    pub logical_msg_id: String,
    pub chat_id: Option<Uuid>,
    pub from_user_id: Option<Uuid>,
    pub from_device_id: Option<Uuid>,
    pub header: Value,
    pub ciphertext: String,
    pub created_at: Option<chrono::DateTime<chrono::Utc>>,
}