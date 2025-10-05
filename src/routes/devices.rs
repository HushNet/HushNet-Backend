use axum::{routing::{get, post}, Router};
use sqlx::PgPool;

use crate::controllers::device_controller;

pub fn routes() -> Router<PgPool> {
    Router::new()
        .route("/devices/create", post(device_controller::create_device))
}