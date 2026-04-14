use sqlx::{Row, sqlite::SqlitePool};

use crate::models::ClientMessage;

pub type DbPool = SqlitePool;

/// 创建数据库连接池
pub async fn create_pool(db_path: &str) -> anyhow::Result<DbPool> {
    let url = format!("sqlite:{}?mode=rwc", db_path);
    let pool = SqlitePool::connect(&url).await?;
    Ok(pool)
}

/// 保存消息到数据库
pub async fn save_message(
    pool: &DbPool,
    username: &str,
    room: &str,
    content: &str,
) -> anyhow::Result<()> {
    sqlx::query("INSERT INTO messages (username, room, content) VALUES (?, ?, ?)")
        .bind(username)
        .bind(room)
        .bind(content)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn get_room_history(
    pool: &DbPool,
    room: &str,
    limit: i64,
) -> anyhow::Result<Vec<ClientMessage>> {
    let rows = sqlx::query(
        "SELECT username, room, content FROM messages WHERE room = ? ORDER BY id DESC LIMIT ?",
    )
    .bind(room)
    .bind(limit)
    .fetch_all(pool)
    .await?;

    let messages: Vec<ClientMessage> = rows
        .into_iter()
        .map(|row| ClientMessage {
            username: row.try_get("username").unwrap_or_default(),
            room: row.try_get("room").unwrap_or_default(),
            content: row.try_get("content").unwrap_or_default(),
        })
        .collect();

    Ok(messages.into_iter().rev().collect())
}
/// 注册用户，成功返回 true，用户名已存在返回 false
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

/// 获取用户的密码哈希
pub async fn get_password_hash(pool: &DbPool, username: &str) -> anyhow::Result<Option<String>> {
    let row = sqlx::query("SELECT password_hash FROM users WHERE username = ?")
        .bind(username)
        .fetch_optional(pool)
        .await?;

    Ok(row.and_then(|r| r.try_get("password_hash").ok()))
}
