use crate::controllers::user_controller;
use axum::{
    routing::{get, post},
    Router,
};
use sqlx::PgPool;

pub fn routes() -> Router<PgPool> {
    Router::new()
        .route("/users", get(user_controller::list_users))
        .route("/users/create", post(user_controller::create_user))
}
