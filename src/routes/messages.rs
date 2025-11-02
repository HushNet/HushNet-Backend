use axum::{
    routing::{get, post},
    Router,
};

use crate::{app_state::AppState, controllers::messages_controller};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/messages", post(messages_controller::send_message))
        .route(
            "/messages/pending",
            get(messages_controller::get_pending_messages),
        )
}
