mod controllers;
mod middlewares;
mod models;
mod repository;
mod routes;
mod services;
use axum::{Extension, Router};
use sqlx::PgPool;
use tokio::sync::broadcast;
use std::net::SocketAddr;
mod app_state;
mod utils;
mod realtime;

use std::env;

use crate::app_state::AppState;
use crate::models::realtime::RealtimeEvent;
use crate::realtime::listener::start_pg_listeners;
#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    tracing_subscriber::fmt::init();
    dotenvy::dotenv().ok();
    let database_url = env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    let pool: sqlx::Pool<sqlx::Postgres> = PgPool::connect(&database_url).await?;
    let jwt_secret = std::env::var("JWT_SECRET").unwrap();

    let state: AppState = AppState { pool: pool.clone(), jwt_secret };
    let (tx, _rx) = broadcast::channel::<RealtimeEvent>(100);
    tokio::spawn(start_pg_listeners(pool.clone(), tx.clone()));


    let app = Router::new()
        .merge(routes::users::routes().with_state(state.clone()))
        .merge(routes::devices::routes().with_state(state.clone()))
        .merge(routes::root::routes().with_state(state.clone()))
        .merge(routes::sessions::routes().with_state(state.clone()))
        .merge(routes::chats::routes().with_state(state.clone()))
        .merge(routes::messages::routes().with_state(state.clone()))
        .merge(routes::websocket::routes()) 
        .layer(Extension(tx));

    let addr = SocketAddr::from(([127, 0, 0, 1], 8080));

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
    Ok(())
}
