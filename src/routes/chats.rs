use axum::{
    routing::{get, post},
    Router,
};

use crate::{app_state::AppState, controllers::chats_controller};

pub fn routes() -> Router<AppState> {
    Router::new().route("/chats", get(chats_controller::get_all_chats))
}
