use sqlx::{PgPool, Result};

pub async fn enrollment_token_exists(pool: &PgPool, token: &str) -> Result<bool> {
    let exists = sqlx::query("SELECT 1 FROM used_tokens WHERE token = $1")
        .bind(token)
        .fetch_optional(pool)
        .await?;
    Ok(exists.is_some())
}

pub async fn add_used_token(pool: &PgPool, token: &str) -> Result<()> {
    sqlx::query!(
        "INSERT INTO used_tokens (token) VALUES ($1)
        ON CONFLICT (token) DO NOTHING",
        token
    )
    .execute(pool)
    .await?;
    Ok(())
}