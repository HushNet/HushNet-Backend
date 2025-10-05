use crate::models::user::User;
use sqlx::{PgPool, Result};

pub async fn get_all_users(pool: &PgPool) -> Result<Vec<User>> {
    let users = sqlx::query_as::<_, User>("SELECT * FROM users")
        .fetch_all(pool)
        .await?;
    Ok(users)
}

pub async fn create_user(pool: &PgPool, username: &str) -> Result<User> {
    let user = sqlx::query_as!(
        User,
        r#"
        INSERT INTO users (username)
        VALUES ($1)
        RETURNING id, username, created_at
        "#,
        username
    )
    .fetch_one(pool)
    .await?;

    Ok(user)
}