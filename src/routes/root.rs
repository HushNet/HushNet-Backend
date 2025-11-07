use crate::{app_state::AppState, controllers::root_controller};
use axum::{
    routing::{get, post},
    Router,
};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/", get(root_controller::root))
        .route("/health", get(root_controller::health_check))
}
