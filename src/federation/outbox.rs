// src/federation/outbox.rs
//
// Background worker that delivers queued outbound S2S messages.
//
// The outbox provides durability for cross-node message delivery: when Node A
// forwards a message to Node B, it first writes the request body to the
// federation_outbox table, then attempts immediate delivery in a spawned task.
// If that attempt fails (Node B is unreachable, times out, etc.), the outbox
// worker picks up the entry on its next poll cycle and retries with exponential
// backoff.
//
// This decouples the client-facing POST /messages response from the S2S
// network call: Node A returns 202 Accepted to the client as soon as the
// entry is written to the outbox, regardless of Node B's availability.
//
// Backoff schedule (seconds):
//   attempt 0 → immediate (spawned task at request time)
//   attempt 1 → 10 s
//   attempt 2 → 20 s
//   attempt 3 → 40 s
//   ...
//   attempt 12+ → 3600 s (1 hour, cap)
//
// After MAX_ATTEMPTS the entry is marked 'failed'. A separate mechanism
// (not implemented here) could push a delivery-failure event to the
// originating client's WebSocket connection.

use std::sync::Arc;
use std::time::Duration;

use sqlx::PgPool;
use tokio::time;

use crate::{
    models::federation::S2sMessagePayload,
    repository::federation_repository,
    utils::node_keys::NodeKeys,
};

use super::client::FederationClient;

const POLL_INTERVAL: Duration = Duration::from_secs(10);
const MAX_ATTEMPTS: i32 = 10;

/// Long-running task: poll the outbox and retry failed deliveries.
///
/// Spawn this once at startup:
/// ```rust
/// tokio::spawn(federation::outbox::run(pool, node_keys, node_id, http));
/// ```
pub async fn run(
    pool: PgPool,
    node_keys: Arc<NodeKeys>,
    this_node_id: String,
    http_client: reqwest::Client,
) {
    let mut interval = time::interval(POLL_INTERVAL);
    // Delay mode: if a tick is missed (the previous iteration took longer than
    // POLL_INTERVAL), skip the missed ticks rather than bursting.
    interval.set_missed_tick_behavior(time::MissedTickBehavior::Delay);

    loop {
        interval.tick().await;

        // Housekeeping: purge nonces older than 5 minutes.
        if let Err(e) = federation_repository::purge_expired_nonces(&pool).await {
            eprintln!("[outbox] nonce purge failed: {e}");
        }

        let entries = match federation_repository::fetch_due_outbox_entries(&pool).await {
            Ok(v) => v,
            Err(e) => {
                eprintln!("[outbox] db error fetching due entries: {e}");
                continue;
            }
        };

        for entry in entries {
            let pool = pool.clone();
            let client = FederationClient::new(
                http_client.clone(),
                node_keys.clone(),
                this_node_id.clone(),
            );

            tokio::spawn(async move {
                let payload: S2sMessagePayload = match serde_json::from_value(entry.payload) {
                    Ok(p) => p,
                    Err(e) => {
                        eprintln!("[outbox] cannot deserialize entry {}: {e}", entry.id);
                        // Malformed entries will never succeed; mark failed immediately.
                        let _ = federation_repository::record_outbox_failure(
                            &pool,
                            entry.id,
                            MAX_ATTEMPTS,
                            MAX_ATTEMPTS,
                        )
                        .await;
                        return;
                    }
                };

                let node =
                    match federation_repository::get_federation_node(&pool, &entry.target_node_id)
                        .await
                    {
                        Ok(Some(n)) => n,
                        Ok(None) => {
                            eprintln!(
                                "[outbox] unknown target node '{}' for entry {}",
                                entry.target_node_id, entry.id
                            );
                            let _ = federation_repository::record_outbox_failure(
                                &pool,
                                entry.id,
                                entry.attempt_count + 1,
                                MAX_ATTEMPTS,
                            )
                            .await;
                            return;
                        }
                        Err(e) => {
                            eprintln!("[outbox] db error looking up node: {e}");
                            return;
                        }
                    };

                match client.forward_messages(&node.api_url, &payload).await {
                    Ok(_) => {
                        let _ = federation_repository::mark_outbox_delivered(&pool, entry.id).await;
                    }
                    Err(e) => {
                        eprintln!(
                            "[outbox] delivery attempt {} for entry {} failed: {e}",
                            entry.attempt_count + 1,
                            entry.id
                        );
                        let _ = federation_repository::record_outbox_failure(
                            &pool,
                            entry.id,
                            entry.attempt_count + 1,
                            MAX_ATTEMPTS,
                        )
                        .await;
                    }
                }
            });
        }
    }
}
