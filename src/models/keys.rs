use serde::{Serialize, Deserialize};
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize)]
pub struct DeviceKeys {
    pub id: Uuid,
    pub device_id: Uuid,
    pub signed_prekey: String,
    pub signed_prekey_sig: String,
    pub one_time_prekeys: serde_json::Value,
    pub created_at: Option<chrono::NaiveDateTime>
}