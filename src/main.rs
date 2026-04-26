mod controllers;
mod federation;
mod middlewares;
mod models;
mod repository;
mod routes;
mod services;
use axum::{Extension, Router};
use sqlx::PgPool;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::broadcast;
mod app_state;
mod realtime;
mod registry;
mod utils;

use std::env;

use crate::app_state::AppState;
use crate::models::realtime::RealtimeEvent;
use crate::realtime::listener::start_pg_listeners;
use crate::utils::node_keys::NodeKeys;
use registry::register::register_with_registry;

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    tracing_subscriber::fmt::init();
    dotenvy::dotenv().ok();

    let database_url = env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    let server_host = env::var("SERVER_HOST").unwrap_or_else(|_| "0.0.0.0".into());
    let server_port = env::var("SERVER_PORT").unwrap_or_else(|_| "8080".into());
    let jwt_secret = env::var("JWT_SECRET").unwrap();

    let registry_url =
        env::var("REGISTRY_URL").unwrap_or_else(|_| "https://registry.hushnet.net".into());
    let node_host = env::var("NODE_HOST").unwrap_or_else(|_| "node-unknown.hushnet.net".into());
    let node_api_url =
        env::var("NODE_API_URL").unwrap_or_else(|_| format!("https://{node_host}/api"));

    let pool: sqlx::Pool<sqlx::Postgres> = PgPool::connect(&database_url).await?;
    let keys = NodeKeys::load_or_generate()?;
    println!("Public key (base64): {}", keys.public_b64);

    if env::var("REGISTER_TO_REGISTRY")
        .unwrap_or_else(|_| "false".into())
        .to_lowercase()
        == "true"
    {
        println!("Registering with registry at {registry_url}");
        register_with_registry(&registry_url).await?;
    }

    let http_client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()?;

    let state: AppState = AppState {
        pool: pool.clone(),
        jwt_secret,
        node_keys: Arc::new(keys),
        this_node_id: node_host.clone(),
        this_api_url: node_api_url,
        registry_url: registry_url.clone(),
        http_client: http_client.clone(),
    };

    let (tx, _rx) = broadcast::channel::<RealtimeEvent>(100);
    tokio::spawn(start_pg_listeners(pool.clone(), tx.clone()));

    // Outbox worker: retries failed cross-node message deliveries.
    tokio::spawn(federation::outbox::run(
        pool.clone(),
        state.node_keys.clone(),
        state.this_node_id.clone(),
        http_client,
    ));

    let app = Router::new()
        .merge(routes::users::routes().with_state(state.clone()))
        .merge(routes::devices::routes().with_state(state.clone()))
        .merge(routes::root::routes().with_state(state.clone()))
        .merge(routes::sessions::routes().with_state(state.clone()))
        .merge(routes::chats::routes().with_state(state.clone()))
        .merge(routes::messages::routes().with_state(state.clone()))
        .merge(routes::federation::routes().with_state(state.clone()))
        .merge(routes::websocket::routes())
        .layer(Extension(tx));

    let addr = SocketAddr::new(server_host.parse().unwrap(), server_port.parse().unwrap());
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
    Ok(())
}
