use crate::{app_state::AppState, controllers::user_controller};
use axum::{
    routing::{get, post},
    Router,
};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/users", get(user_controller::list_users))
        .route("/users/create", post(user_controller::create_user))
}
