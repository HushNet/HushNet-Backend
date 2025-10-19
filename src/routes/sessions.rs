use crate::{app_state::AppState, controllers::session_controller};
use axum::{
    middleware, routing::{get, post}, Router
};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/sessions", post(session_controller::create_session))
}
