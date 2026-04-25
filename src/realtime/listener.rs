use serde_json::Value;
use sqlx::{postgres::PgListener, PgPool};
use tokio::sync::broadcast;
use tracing::{error, info, warn};

use crate::models::realtime::RealtimeEvent;

pub async fn start_pg_listeners(pool: PgPool, tx: broadcast::Sender<RealtimeEvent>) {
    let mut listener = PgListener::connect_with(&pool).await.unwrap();
    listener
        .listen_all(vec![
            "messages_channel",
            "sessions_channel",
            "pending_sessions_channel",
            "devices_channel",
        ])
        .await
        .unwrap();

    info!("PG listener started, watching 4 channels");

    loop {
        match listener.recv().await {
            Ok(notif) => {
                let channel = notif.channel();
                match serde_json::from_str::<Value>(notif.payload()) {
                    Ok(payload) => {
                        let event_type = payload
                            .get("type")
                            .and_then(|v| v.as_str())
                            .unwrap_or("unknown");
                        let user_id = payload
                            .get("user_id")
                            .and_then(|v| v.as_str())
                            .unwrap_or("?");

                        info!(
                            %channel,
                            %event_type,
                            %user_id,
                            subscribers = tx.receiver_count(),
                            "PG notify received, forwarding to broadcast"
                        );

                        let event = RealtimeEvent {
                            event_type: event_type.to_string(),
                            payload,
                        };
                        if let Err(e) = tx.send(event) {
                            warn!(%channel, err = %e, "broadcast send failed (no subscribers?)");
                        }
                    }
                    Err(e) => {
                        warn!(%channel, err = %e, "PG notify payload parse failed");
                    }
                }
            }
            Err(e) => {
                error!(err = %e, "PG listener error");
            }
        }
    }
}
