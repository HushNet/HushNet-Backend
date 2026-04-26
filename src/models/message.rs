use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow, Clone)]
#[allow(dead_code)]
pub struct Message {
    pub id: Uuid,
    pub logical_msg_id: String,
    pub chat_id: Uuid,
    pub from_user_id: Uuid,
    pub from_device_id: Uuid,
    pub to_user_id: Uuid,
    pub to_device_id: Uuid,
    pub header: Value,      // Double Ratchet header (JSON)
    pub ciphertext: String, // base64(nonce || cipher || mac)
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
///
/// For local delivery, set `to_user_id`.
/// For cross-node delivery, also set `to_user_address` ("bob@node-b.hushnet.net").
/// When `to_user_address` points to a remote node, `to_user_id` is ignored by
/// the server and the message is forwarded via S2S. Existing clients that do
/// not send `to_user_address` continue to work unchanged.
#[derive(Debug, Deserialize)]
pub struct OutgoingMessage {
    pub chat_id: Uuid,
    pub logical_msg_id: String,
    pub to_user_id: Uuid,
    /// Optional federated address for cross-node delivery.
    /// Format: "username@node-host" (e.g. "bob@node-b.hushnet.net").
    #[serde(default)]
    pub to_user_address: Option<String>,
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
