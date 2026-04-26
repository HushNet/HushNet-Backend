use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

// ─── Peer node record ────────────────────────────────────────────────────────

/// A peer node as stored in the federation_nodes table.
/// Rows are created lazily on first contact (via registry lookup) and cached.
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct FederationNode {
    pub id: Uuid,
    /// Canonical host-based identifier: "node-a.hushnet.net"
    pub node_id: String,
    /// Base API URL the S2S client uses for outbound requests.
    pub api_url: String,
    /// Ed25519 verifying key (base64) for authenticating inbound S2S requests.
    pub public_key_b64: String,
    pub last_seen: Option<DateTime<Utc>>,
    pub is_blocked: bool,
    pub created_at: DateTime<Utc>,
}

// ─── Outbox entry ────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct FederationOutboxEntry {
    pub id: Uuid,
    pub target_node_id: String,
    pub logical_msg_id: String,
    /// Verbatim JSON body to POST to /s2s/messages on the target node.
    pub payload: Value,
    pub attempt_count: i32,
    pub last_attempt: Option<DateTime<Utc>>,
    pub next_attempt: DateTime<Utc>,
    /// "pending" | "delivered" | "failed"
    pub status: String,
    pub created_at: DateTime<Utc>,
}

// ─── S2S wire types ──────────────────────────────────────────────────────────

/// Body of POST /s2s/messages.
///
/// Sent by Node A to Node B to deliver one logical message to a local user.
/// Each entry in `payloads` is encrypted specifically for one recipient device;
/// Node B stores each as an independent row in the messages table.
///
/// `from_identity_pubkey` is included so Node B can upsert the shadow device
/// row (devices table) without requiring a round-trip back to Node A. Shadow
/// devices need a valid identity_pubkey but no actual prekey material.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct S2sMessagePayload {
    /// Shared across all device fanouts of this message. Used for idempotent
    /// delivery: Node B rejects duplicates keyed on (logical_msg_id, to_device_id).
    pub logical_msg_id: String,
    /// "alice@node-a.hushnet.net" — used to upsert the shadow user on Node B.
    pub from_federated_address: String,
    /// UUID of the sending device, authoritative on Node A.
    pub from_device_id: Uuid,
    /// Ed25519 identity public key of the sending device (base64).
    pub from_identity_pubkey: String,
    /// Local username of the recipient on Node B.
    pub to_user: String,
    pub payloads: Vec<S2sDevicePayload>,
}

/// One ciphertext destined for a single recipient device.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct S2sDevicePayload {
    pub to_device_id: Uuid,
    pub header: Value,
    pub ciphertext: String,
}

/// Body of POST /s2s/sessions.
///
/// Sent by Node A to Node B to forward an X3DH session initiation.
/// Node B inserts the data into pending_sessions so the local recipient
/// sees it via GET /sessions/pending or the WebSocket stream.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct S2sSessionPayload {
    pub from_federated_address: String,
    pub from_device_id: Uuid,
    pub from_identity_pubkey: String,
    /// Local username of the recipient on Node B.
    pub to_user: String,
    pub sessions_init: Vec<S2sSessionInit>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct S2sSessionInit {
    pub recipient_device_id: Uuid,
    pub ephemeral_pubkey: String,
    pub sender_prekey_pub: String,
    pub otpk_used: String,
    pub ciphertext: String,
}

/// Body of POST /s2s/ack (Node B → Node A).
///
/// Advisory: the outbox worker already marks entries delivered when it receives
/// a 2xx from forward_messages, so this ack is redundant in the happy path.
/// It exists as an explicit signal for cases where Node B wants to proactively
/// confirm delivery without waiting for Node A to poll.
#[derive(Debug, Serialize, Deserialize)]
pub struct S2sAck {
    pub logical_msg_id: String,
    /// "delivered" | "duplicate"
    pub status: String,
}

/// Response body for GET /s2s/info.
///
/// Used by peers during bootstrapping to obtain this node's public key before
/// the registry has been consulted. The caller must still verify the returned
/// key against the registry to prevent a MITM from substituting its own key.
#[derive(Debug, Serialize, Deserialize)]
pub struct NodeInfo {
    pub node_id: String,
    pub api_url: String,
    pub public_key_b64: String,
    pub protocol_version: &'static str,
}
