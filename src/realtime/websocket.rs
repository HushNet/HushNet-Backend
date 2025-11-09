use axum::{
    extract::{
        ws::{Message, WebSocket},
        Path, WebSocketUpgrade,
    },
    response::IntoResponse,
    Extension,
};

use tokio::sync::broadcast;

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

    println!("WS connected for user {}", user_id);

    tokio::spawn(async move {
        while let Ok(event) = rx.recv().await {
            let payload_user = event
                .payload
                .get("user_id")
                .and_then(|v| v.as_str())
                .unwrap_or_default();

            if payload_user == user_id {
                if let Ok(json) = serde_json::to_string(&event) {
                    if socket.send(Message::Text(json)).await.is_err() {
                        break;
                    }
                }
            }
        }
        println!("WS closed for user {}", user_id);
    });
}
