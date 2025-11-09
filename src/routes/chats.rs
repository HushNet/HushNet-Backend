use axum::{routing::get, Router};

use crate::{app_state::AppState, controllers::chats_controller};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/chats", get(chats_controller::get_all_chats))
        .route("/chats/:id", get(chats_controller::get_all_chats))
}
