use axum::{
    routing::{get, post},
    Router,
};
use sqlx::PgPool;

use crate::controllers::device_controller;

pub fn routes() -> Router<PgPool> {
    Router::new()
        .route("/users/:id/devices", post(device_controller::create_device))
        .route(
            "/users/:id/devices",
            get(device_controller::get_devices_for_user),
        )
}
