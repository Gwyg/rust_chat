use crate::db::DbPool;
use crate::models::ClientMessage;
use sqlx::Row;

pub struct PaginatedMessages {
    pub messages: Vec<ClientMessage>,
    pub min_id: Option<i64>,
    pub has_more: bool,
}
/// 撤回消息结果
pub struct RecallResult {
    pub sender: String,
    pub room: String,
}

pub async fn recall_group_message(
    pool: &DbPool,
    message_id: i64,
    group_id: &str,
    username: &str,
) -> anyhow::Result<RecallResult> {
    let row = sqlx::query(
        "SELECT id, sender, group_id as room, content, created_at, recalled 
         FROM group_messages WHERE id = ? AND group_id = ?",
    )
    .bind(message_id)
    .bind(group_id)
    .fetch_optional(pool)
    .await?;

    let row = match row {
        Some(r) => r,
        None => return Err(anyhow::anyhow!("消息不存在")),
    };

    let sender: String = row.try_get("sender").unwrap_or_default();
    let room: String = row.try_get("room").unwrap_or_default();
    let recalled: bool = row.try_get("recalled").unwrap_or(false);
    let created_at = get_created_at(&row).unwrap_or_else(|_| chrono::DateTime::UNIX_EPOCH.naive_utc());


    if sender != username {
        return Err(anyhow::anyhow!("只能撤回自己的消息"));
    }
    if recalled {
        return Err(anyhow::anyhow!("消息已撤回"));
    }

    let now = chrono::Utc::now().naive_utc();
    let elapsed = (now - created_at).num_seconds();
    if elapsed > 120 {
        return Err(anyhow::anyhow!("超过 2 分钟，无法撤回"));
    }

    sqlx::query("UPDATE group_messages SET recalled = 1 WHERE id = ?")
        .bind(message_id)
        .execute(pool)
        .await?;

    Ok(RecallResult { sender, room })
}

pub async fn recall_private_message(
    pool: &DbPool,
    message_id: i64,
    conv_id: &str,
    username: &str,
) -> anyhow::Result<RecallResult> {
    let row = sqlx::query(
        "SELECT id, sender, conv_id as room, content, created_at, recalled 
         FROM private_messages WHERE id = ? AND conv_id = ?",
    )
    .bind(message_id)
    .bind(conv_id)
    .fetch_optional(pool)
    .await?;

    let row = match row {
        Some(r) => r,
        None => return Err(anyhow::anyhow!("消息不存在")),
    };

    let sender: String = row.try_get("sender").unwrap_or_default();
    let room: String = row.try_get("room").unwrap_or_default();
    let recalled: bool = row.try_get("recalled").unwrap_or(false);
    let created_at = get_created_at(&row).unwrap_or_else(|_| chrono::DateTime::UNIX_EPOCH.naive_utc());

    if sender != username {
        return Err(anyhow::anyhow!("只能撤回自己的消息"));
    }
    if recalled {
        return Err(anyhow::anyhow!("消息已撤回"));
    }

    let now = chrono::Utc::now().naive_utc();
    let elapsed = (now - created_at).num_seconds();
    if elapsed > 120 {
        return Err(anyhow::anyhow!("超过 2 分钟，无法撤回"));
    }

    sqlx::query("UPDATE private_messages SET recalled = 1 WHERE id = ?")
        .bind(message_id)
        .execute(pool)
        .await?;

    Ok(RecallResult { sender, room })
}
pub async fn save_message(
    pool: &DbPool,
    username: &str,
    room: &str,
    content: &str,
) -> anyhow::Result<()> {
    // 改为：
    sqlx::query("INSERT INTO group_messages (group_id, sender, content) VALUES (?, ?, ?)")
        .bind(room) // group_id
        .bind(username) // sender
        .bind(content)
        .execute(pool)
        .await?;
    Ok(())
}
fn get_created_at(row: &sqlx::sqlite::SqliteRow) -> anyhow::Result<chrono::NaiveDateTime> {
    // 尝试作为 i64 (Unix timestamp) 获取
    if let Ok(ts) = row.try_get::<i64, _>("created_at") {
        return chrono::DateTime::from_timestamp(ts, 0)
            .map(|dt| dt.naive_utc())
            .ok_or_else(|| anyhow::anyhow!("Invalid timestamp"));
    }
    
    // 尝试作为 String (ISO 8601) 获取
    let time_str: String = row.try_get("created_at")?;
    // 解析 ISO 8601 格式，例如 "2023-10-01T12:00:00"
    chrono::NaiveDateTime::parse_from_str(&time_str, "%Y-%m-%dT%H:%M:%S%.f")
        .or_else(|_| chrono::NaiveDateTime::parse_from_str(&time_str, "%Y-%m-%d %H:%M:%S%.f"))
        .map_err(|e| anyhow::anyhow!("Failed to parse date: {}", e))
}

pub async fn get_room_history_paginated(
    pool: &DbPool,
    room: &str,
    before_id: Option<i64>,
    limit: i64,
) -> anyhow::Result<PaginatedMessages> {
    // 先查实际有多少条
    let actual_limit = limit + 1;
    let rows = if let Some(before) = before_id {
        sqlx::query(
            "SELECT id, sender, group_id as room, content, recalled FROM group_messages 
            WHERE group_id = ? AND id < ? ORDER BY id DESC LIMIT ?",
        )
        .bind(room)
        .bind(before)
        .bind(actual_limit)
        .fetch_all(pool)
        .await?
    } else {
        sqlx::query(
            "SELECT id, sender, group_id as room, content, recalled FROM group_messages 
            WHERE group_id = ? ORDER BY id DESC LIMIT ?",
        )
        .bind(room)
        .bind(actual_limit)
        .fetch_all(pool)
        .await?
    };

    let has_more = (rows.len() as i64) > limit;
    let page_rows = if has_more {
        &rows[..(limit as usize)]
    } else {
        &rows[..]
    };

    let mut messages: Vec<ClientMessage> = page_rows
        .iter()
        .map(|row| {
            let id: i64 = row.try_get("id").unwrap_or(0);
            ClientMessage {
                username: row.try_get("sender").unwrap_or_default(),
                room: row.try_get("room").unwrap_or_default(),
                content: row.try_get("content").unwrap_or_default(),
                msg_type: format!("message:{}", id),
                recalled: row.try_get("recalled").unwrap_or(false),
                message_id: Some(id),
                ..Default::default()
            }
        })
        .collect();
    messages.reverse();

    let min_id = if messages.is_empty() {
        None
    } else {
        Some(
            page_rows
                .iter()
                .map(|r| {
                    let id: i64 = r.try_get("id").unwrap_or(0);
                    id
                })
                .min()
                .unwrap_or(0),
        )
    };

    Ok(PaginatedMessages {
        messages,
        min_id,
        has_more,
    })
}
