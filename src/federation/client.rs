// src/federation/client.rs
//
// Authenticated HTTP client for outbound S2S requests.
//
// Every outbound request is signed with this node's Ed25519 private key so
// that the receiving node can verify the sender's identity against the public
// key stored in the central registry.
//
// Canonical string signed (UTF-8, fields separated by "\n"):
//
//   {HTTP_METHOD}\n{path}\n{timestamp}\n{nonce}
//
// The path component is extracted from the full URL by stripping the scheme
// and authority, making it consistent with what the receiver reconstructs from
// the incoming request URI.  Only the path+query portion is signed, not the
// host, so that node API URLs can change without invalidating the signing logic.
//
// Four headers carry the authentication material:
//
//   X-Node-ID        — this node's canonical identifier
//   X-Timestamp      — Unix seconds (string)
//   X-Nonce          — 16 random bytes, base64-encoded
//   X-Node-Signature — Ed25519(canonical), base64-encoded

use std::sync::Arc;

use anyhow::{Context, Result};
use base64::{engine::general_purpose::STANDARD as B64, Engine};
use ed25519_dalek::Signer;
use reqwest::Client;
use serde::Serialize;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::{
    models::{
        device::DeviceBundle,
        federation::{NodeInfo, S2sAck, S2sMessagePayload, S2sSessionPayload},
    },
    utils::node_keys::NodeKeys,
};

/// HTTP client for outbound S2S communication.
///
/// Clone is cheap: both `http` (reqwest::Client) and `node_keys` (Arc) are
/// reference-counted internally.
#[derive(Clone)]
pub struct FederationClient {
    pub http: Client,
    node_keys: Arc<NodeKeys>,
    pub this_node_id: String,
}

impl FederationClient {
    pub fn new(http: Client, node_keys: Arc<NodeKeys>, this_node_id: String) -> Self {
        Self {
            http,
            node_keys,
            this_node_id,
        }
    }

    /// Fetch the prekey bundle for `username` from a peer node.
    ///
    /// The returned Vec has one DeviceBundle per device registered for that
    /// user on the remote node. One-time prekeys are consumed by the remote
    /// node on fetch (same semantics as the local GET /users/:id/keys endpoint).
    pub async fn fetch_peer_keys(
        &self,
        api_url: &str,
        username: &str,
    ) -> Result<Vec<DeviceBundle>> {
        let url = format!("{api_url}/s2s/users/{username}/keys");
        self.signed_get(&url)
            .await?
            .error_for_status()
            .context("peer returned error for key fetch")?
            .json::<Vec<DeviceBundle>>()
            .await
            .context("invalid key bundle in peer response")
    }

    /// Forward an X3DH session initiation to the peer that hosts the recipient.
    pub async fn forward_session(&self, api_url: &str, payload: &S2sSessionPayload) -> Result<()> {
        self.signed_post(api_url, "/s2s/sessions", payload)
            .await?
            .error_for_status()
            .context("peer rejected session forward")?;
        Ok(())
    }

    /// Forward a batch of device-specific ciphertexts to the peer.
    /// Returns the S2sAck the receiving node sends back.
    pub async fn forward_messages(
        &self,
        api_url: &str,
        payload: &S2sMessagePayload,
    ) -> Result<S2sAck> {
        self.signed_post(api_url, "/s2s/messages", payload)
            .await?
            .error_for_status()
            .context("peer rejected message forward")?
            .json::<S2sAck>()
            .await
            .context("invalid ack in peer response")
    }

    // ── Private helpers ───────────────────────────────────────────────────────

    async fn signed_get(&self, url: &str) -> Result<reqwest::Response> {
        let path = url_path(url);
        let (ts, nonce, sig) = self.sign("GET", path)?;
        self.http
            .get(url)
            .header("X-Node-ID", &self.this_node_id)
            .header("X-Timestamp", &ts)
            .header("X-Nonce", &nonce)
            .header("X-Node-Signature", &sig)
            .send()
            .await
            .context("S2S GET request failed")
    }

    async fn signed_post<T: Serialize>(
        &self,
        api_url: &str,
        path: &str,
        body: &T,
    ) -> Result<reqwest::Response> {
        let (ts, nonce, sig) = self.sign("POST", path)?;
        let url = format!("{api_url}{path}");
        self.http
            .post(&url)
            .header("Content-Type", "application/json")
            .header("X-Node-ID", &self.this_node_id)
            .header("X-Timestamp", &ts)
            .header("X-Nonce", &nonce)
            .header("X-Node-Signature", &sig)
            .json(body)
            .send()
            .await
            .context("S2S POST request failed")
    }

    /// Build the canonical string and sign it with this node's private key.
    ///
    /// canonical = "{method}\n{path}\n{ts}\n{nonce}"
    fn sign(&self, method: &str, path: &str) -> Result<(String, String, String)> {
        let ts = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs()
            .to_string();

        let nonce = {
            use ed25519_dalek::ed25519::signature::rand_core::{OsRng, RngCore};
            let mut buf = [0u8; 16];
            OsRng.fill_bytes(&mut buf);
            B64.encode(buf)
        };

        let canonical = format!("{method}\n{path}\n{ts}\n{nonce}");
        let signing_key = self.node_keys.signing_key()?;
        let signature = signing_key.sign(canonical.as_bytes());
        let sig_b64 = B64.encode(signature.to_bytes());

        Ok((ts, nonce, sig_b64))
    }
}

/// Extract the path+query portion from a full URL.
///
/// "https://node-a.hushnet.net/api/s2s/messages?x=1" → "/api/s2s/messages?x=1"
///
/// This is what the receiving node reconstructs from the incoming request URI,
/// so both sides of the signature use the same string.
fn url_path(url: &str) -> &str {
    if let Some(pos) = url.find("://") {
        let after_scheme = &url[pos + 3..];
        if let Some(slash) = after_scheme.find('/') {
            return &after_scheme[slash..];
        }
        return "/";
    }
    url
}
