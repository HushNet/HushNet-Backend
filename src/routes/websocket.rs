use crate::realtime::websocket::ws_route;
use axum::{routing::get, Router};

pub fn routes() -> Router {
    Router::new().route("/ws/:user_id", get(ws_route))
}
