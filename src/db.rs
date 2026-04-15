use sqlx::{Row, sqlite::SqlitePool};

use crate::models::{ClientMessage, ConversationItem};

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
    sqlx::query("INSERT INTO messages (username, room, content, conversation_id) VALUES (?, ?, ?, NULL)")
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
            msg_type: "message".into(),
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

/// 获取或创建私聊会话，返回 conv_id
pub async fn get_or_create_private_conv(
    pool: &DbPool,
    user_a: &str,
    user_b: &str,
) -> anyhow::Result<String> {
    // 排序保证唯一性
    let (a, b) = if user_a < user_b {
        (user_a, user_b)
    } else {
        (user_b, user_a)
    };
    let conv_id = format!("{}_{}", a, b);

    sqlx::query(
        "INSERT OR IGNORE INTO conversations (conv_id, type) VALUES (?, 'private')"
    )
    .bind(&conv_id)
    .execute(pool)
    .await?;

    Ok(conv_id)
}

/// 保存私聊消息（写入 messages 表，带 conversation_id）
pub async fn save_private_message(
    pool: &DbPool,
    username: &str,
    conv_id: &str,
    content: &str,
) -> anyhow::Result<()> {
    sqlx::query(
        "INSERT INTO messages (username, room, content, conversation_id) VALUES (?, ?, ?, ?)"
    )
    .bind(username)
    .bind(conv_id)
    .bind(content)
    .bind(conv_id)
    .execute(pool)
    .await?;
    Ok(())
}

/// 获取私聊历史
pub async fn get_private_history(
    pool: &DbPool,
    conv_id: &str,
    limit: i64,
) -> anyhow::Result<Vec<ClientMessage>> {
    let rows = sqlx::query(
        "SELECT username, room as conv_id, content FROM messages
         WHERE conversation_id = ? ORDER BY id DESC LIMIT ?"
    )
    .bind(conv_id)
    .bind(limit)
    .fetch_all(pool)
    .await?;

    let messages: Vec<ClientMessage> = rows
        .into_iter()
        .map(|row| ClientMessage {
            username: row.try_get("username").unwrap_or_default(),
            room: row.try_get("conv_id").unwrap_or_default(),
            content: row.try_get("content").unwrap_or_default(),
            msg_type: "private".into(),
        })
        .collect();

    Ok(messages.into_iter().rev().collect())
}

/// 获取用户的会话列表（群聊 + 私聊）
pub async fn get_user_conversations(
    pool: &DbPool,
    username: &str,
) -> anyhow::Result<Vec<ConversationItem>> {
    // 这里简化实现，返回所有有消息的 conversation
    let rows = sqlx::query(
        "SELECT conversation_id as conv_id,
                MAX(content) as last_content,
                MAX(id) as last_id
         FROM messages
         WHERE conversation_id IS NOT NULL
           AND conversation_id IN (
               SELECT conv_id FROM conversations WHERE type = 'private'
               AND conv_id LIKE ?
           )
         GROUP BY conversation_id
         ORDER BY last_id DESC"
    )
    .bind(&format!("%{}%", username))
    .fetch_all(pool)
    .await?;

    let mut items = Vec::new();
    for row in rows {
        let conv_id: String = row.try_get("conv_id").unwrap_or_default();
        let name = conv_id
            .split('_')
            .filter(|n| *n != username)
            .next()
            .unwrap_or(&conv_id)
            .to_string();
        items.push(ConversationItem {
            conv_id,
            conv_type: "private".into(),
            name,
            last_content: row.try_get("last_content").ok(),
            last_time: None,
        });
    }

    // 群聊也加入
    let group_rows = sqlx::query(
        "SELECT room as conv_id, MAX(content) as last_content
         FROM messages
         WHERE conversation_id IS NULL
         GROUP BY room
         ORDER BY MAX(id) DESC"
    )
    .fetch_all(pool)
    .await?;

    for row in group_rows {
        items.push(ConversationItem {
            conv_id: row.try_get("conv_id").unwrap_or_default(),
            conv_type: "group".into(),
            name: row.try_get("conv_id").unwrap_or_default(),
            last_content: row.try_get("last_content").ok(),
            last_time: None,
        });
    }

    Ok(items)
}
