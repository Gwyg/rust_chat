use sqlx::sqlite::SqlitePool;

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
    sqlx::query(
        "INSERT INTO messages (username, room, content) VALUES (?, ?, ?)",
    )
    .bind(username)
    .bind(room)
    .bind(content)
    .execute(pool)
    .await?;
    Ok(())
}