use sqlx::Row;
use crate::db::DbPool;

pub async fn register_user(
    pool: &DbPool,
    username: &str,
    password_hash: &str,
) -> anyhow::Result<bool> {
    let result = sqlx::query("INSERT INTO users (username, password_hash) VALUES (?, ?)")
        .bind(username)
        .bind(password_hash)
        .execute(pool)
        .await;

    match result {
        Ok(_) => Ok(true),
        Err(sqlx::Error::Database(e)) if e.message().contains("UNIQUE") => Ok(false),
        Err(e) => Err(e.into()),
    }
}

pub async fn get_all_users(pool: &DbPool) -> anyhow::Result<Vec<String>> {
    let rows = sqlx::query("SELECT username FROM users ORDER BY username ASC")
        .fetch_all(pool)
        .await?;
    Ok(rows.iter().map(|r| r.try_get("username").unwrap_or_default()).collect())
}

pub async fn get_password_hash(pool: &DbPool, username: &str) -> anyhow::Result<Option<String>> {
    let row = sqlx::query("SELECT password_hash FROM users WHERE username = ?")
        .bind(username)
        .fetch_optional(pool)
        .await?;

    Ok(row.and_then(|r| r.try_get("password_hash").ok()))
}