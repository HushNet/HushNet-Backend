use axum::{
    extract::{
        ws::{Message, WebSocket},
        Path, WebSocketUpgrade,
    },
    response::IntoResponse,
    Extension,
};

use tokio::sync::broadcast;
use tracing::{info, warn};

use crate::models::realtime::RealtimeEvent;

pub async fn ws_route(
    Path(user_id): Path<String>,
    ws: WebSocketUpgrade,
    Extension(tx): Extension<broadcast::Sender<RealtimeEvent>>,
) -> impl IntoResponse {
    ws.on_upgrade(|socket| handle_socket(socket, tx, user_id))
}

async fn handle_socket(
    mut socket: WebSocket,
    tx: broadcast::Sender<RealtimeEvent>,
    user_id: String,
) {
    let mut rx = tx.subscribe();

    info!(%user_id, subscribers = tx.receiver_count(), "WS connected");

    tokio::spawn(async move {
        loop {
            match rx.recv().await {
                Ok(event) => {
                    let payload_user = event
                        .payload
                        .get("user_id")
                        .and_then(|v| v.as_str())
                        .unwrap_or_default();

                    if payload_user == user_id {
                        info!(
                            %user_id,
                            event_type = %event.event_type,
                            "WS dispatching event to client"
                        );
                        if let Ok(json) = serde_json::to_string(&event) {
                            if socket.send(Message::Text(json.into())).await.is_err() {
                                info!(%user_id, "WS send failed, closing");
                                break;
                            }
                        }
                    }
                }
                Err(broadcast::error::RecvError::Lagged(n)) => {
                    warn!(%user_id, skipped = n, "WS broadcast lagged, events dropped");
                }
                Err(broadcast::error::RecvError::Closed) => {
                    info!(%user_id, "WS broadcast channel closed");
                    break;
                }
            }
        }
        info!(%user_id, "WS disconnected");
    });
}
