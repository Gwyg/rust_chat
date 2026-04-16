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

/// 获取所有用户名列表
pub async fn get_all_users(pool: &DbPool) -> anyhow::Result<Vec<String>> {
    let rows = sqlx::query("SELECT username FROM users ORDER BY username ASC")
        .fetch_all(pool)
        .await?;
    Ok(rows.iter().map(|r| r.try_get("username").unwrap_or_default()).collect())
}

/// 获取用户的密码哈希
pub async fn get_password_hash(pool: &DbPool, username: &str) -> anyhow::Result<Option<String>> {
    let row = sqlx::query("SELECT password_hash FROM users WHERE username = ?")
        .bind(username)
        .fetch_optional(pool)
        .await?;

    Ok(row.and_then(|r| r.try_get("password_hash").ok()))
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

// ── 好友系统 ────────────────────────────

/// 发送好友申请（已有则忽略）
pub async fn send_friend_request(
    pool: &DbPool,
    from: &str,
    to: &str,
) -> anyhow::Result<()> {
    sqlx::query(
        "INSERT OR IGNORE INTO friendships (from_user, to_user, status) VALUES (?, ?, 'pending')"
    )
    .bind(from)
    .bind(to)
    .execute(pool)
    .await?;
    Ok(())
}

/// 接受好友申请
pub async fn accept_friend(
    pool: &DbPool,
    from: &str,   // 申请发起人
    to: &str,     // 当前用户（接受方）
) -> anyhow::Result<()> {
    sqlx::query(
        "UPDATE friendships SET status = 'accepted' WHERE from_user = ? AND to_user = ?"
    )
    .bind(from)
    .bind(to)
    .execute(pool)
    .await?;
    Ok(())
}

/// 删除好友 / 拒绝申请（双向删除）
pub async fn delete_friend(
    pool: &DbPool,
    user_a: &str,
    user_b: &str,
) -> anyhow::Result<()> {
    sqlx::query(
        "DELETE FROM friendships WHERE (from_user = ? AND to_user = ?) OR (from_user = ? AND to_user = ?)"
    )
    .bind(user_a).bind(user_b)
    .bind(user_b).bind(user_a)
    .execute(pool)
    .await?;
    Ok(())
}

/// 获取好友列表（含待处理申请）
pub async fn get_friends(
    pool: &DbPool,
    username: &str,
) -> anyhow::Result<Vec<crate::models::FriendItem>> {
    // 我发出的申请
    let sent = sqlx::query(
        "SELECT to_user as other, status FROM friendships WHERE from_user = ?"
    )
    .bind(username)
    .fetch_all(pool)
    .await?;

    // 我收到的申请
    let recv = sqlx::query(
        "SELECT from_user as other, status FROM friendships WHERE to_user = ?"
    )
    .bind(username)
    .fetch_all(pool)
    .await?;

    let mut items = vec![];

    for row in sent {
        let other: String = row.try_get("other").unwrap_or_default();
        let status: String = row.try_get("status").unwrap_or_default();
        items.push(crate::models::FriendItem {
            username: other,
            status: if status == "accepted" {
                "accepted".into()
            } else {
                "pending_send".into()   // 我发出、对方未处理
            },
        });
    }

    for row in recv {
        let other: String = row.try_get("other").unwrap_or_default();
        let status: String = row.try_get("status").unwrap_or_default();
        if status == "accepted" {
            // 双向已接受，可能在 sent 里已有，跳过重复
            if !items.iter().any(|i| i.username == other) {
                items.push(crate::models::FriendItem {
                    username: other,
                    status: "accepted".into(),
                });
            }
        } else {
            items.push(crate::models::FriendItem {
                username: other,
                status: "pending_recv".into(),  // 对方发来、我未处理
            });
        }
    }

    Ok(items)
}
