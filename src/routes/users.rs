use crate::{app_state::AppState, controllers::user_controller};
use axum::{
    routing::{get, post},
    Router,
};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/users", get(user_controller::list_users))
        .route("/users/:id", get(user_controller::get_user_by_id))
        .route("/users/create", post(user_controller::create_user))
        .route("/users/login", post(user_controller::login_user))
}
