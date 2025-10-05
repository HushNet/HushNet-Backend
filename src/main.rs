mod models;
mod repository;
mod controllers;
mod routes;

use axum::Router;
use sqlx::PgPool;
use std::net::SocketAddr;

use std::env;
#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    tracing_subscriber::fmt::init();
    dotenvy::dotenv().ok();
    let database_url = env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    let pool: sqlx::Pool<sqlx::Postgres> = PgPool::connect(&database_url).await?;

    let app: Router = routes::users::routes().with_state(pool);
    let addr = SocketAddr::from(([127, 0, 0, 1], 8080));

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
    Ok(())
}