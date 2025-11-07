use anyhow::Result;
use base64::{engine::general_purpose::STANDARD as B64, Engine};
use ed25519_dalek::Signer;
use reqwest::Client;
use serde_json::json;

use crate::utils::node_keys::NodeKeys;

pub async fn register_with_registry(registry_url: &str) -> Result<()> {
    let keys = NodeKeys::load_or_generate()?;
    let client = Client::new();
    let node_name = std::env::var("NODE_NAME").unwrap_or_else(|_| "node-unknown".into());
    let node_host =
        std::env::var("NODE_HOST").unwrap_or_else(|_| "node-unknown.hushnet.net".into());
    let node_api_url =
        std::env::var("NODE_API_URL").unwrap_or_else(|_| format!("https://{}/api", node_host));
    let contact_email = std::env::var("CONTACT_EMAIL").unwrap_or_else(|_| "ops@hushnet.net".into());
    let challenge_res = client
        .post(format!("{registry_url}/api/registry/challenge"))
        .json(&json!({ "pubkey_b64": keys.public_b64 }))
        .send()
        .await?
        .error_for_status()?
        .json::<serde_json::Value>()
        .await?;
    let nonce = challenge_res["nonce"].as_str().unwrap();
    println!("Got nonce: {nonce}");

    let payload = json!({
        "name": node_name,
        "host": node_host,
        "api_base_url": node_api_url,
        "protocol_version": "0.0.1",
        "features": {},
        "contact_email": contact_email
    });

    let canon = serde_json::to_string(&payload)?;
    let message = [canon.as_bytes(), nonce.as_bytes()].concat();

    let signing_key = keys.signing_key()?;
    let sig = signing_key.sign(&message);
    let sig_b64 = B64.encode(sig.to_bytes());

    let register_res = client
        .post(format!("{registry_url}/api/registry/register"))
        .json(&json!({
            "nonce": nonce,
            "pubkey_b64": keys.public_b64,
            "signature_b64": sig_b64,
            "payload": payload
        }))
        .send()
        .await?
        .text()
        .await?;

    println!("Registry response: {register_res}");
    Ok(())
}
