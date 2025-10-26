use axum::{Router, routing::get};
use crate::realtime::websocket::ws_route;

pub fn routes() -> Router {
    Router::new()
        .route("/ws/:user_id", get(ws_route))
}