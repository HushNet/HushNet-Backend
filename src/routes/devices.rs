use axum::{
    routing::{get, post},
    Router,
};

use crate::{app_state::AppState, controllers::device_controller};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/users/:id/devices", post(device_controller::create_device))
        .route(
            "/users/:id/devices",
            get(device_controller::get_devices_for_user),
        )
}
