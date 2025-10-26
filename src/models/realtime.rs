use serde::{Serialize, Deserialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RealtimeEvent {
    pub event_type: String, // "message" | "session" | "device"
    pub payload: serde_json::Value,
}
