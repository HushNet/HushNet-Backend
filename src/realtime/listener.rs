use sqlx::{postgres::PgListener, PgPool};
use tokio::sync::broadcast;
use serde_json::Value;

use crate::models::realtime::RealtimeEvent;

pub async fn start_pg_listeners(pool: PgPool, tx: broadcast::Sender<RealtimeEvent>) {
    let mut listener = PgListener::connect_with(&pool).await.unwrap();
    listener.listen_all(vec![
        "messages_channel",
        "sessions_channel",
        "devices_channel",
    ]).await.unwrap();

    println!("👂 Listening to Postgres realtime channels...");

    while let Ok(notif) = listener.recv().await {
        if let Ok(payload) = serde_json::from_str::<Value>(notif.payload()) {
            if let Some(event_type) = payload.get("type").and_then(|v| v.as_str()) {
                let event = RealtimeEvent {
                    event_type: event_type.to_string(),
                    payload,
                };
                let _ = tx.send(event);
            }
        }
    }
}
