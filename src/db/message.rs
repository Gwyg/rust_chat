use crate::db::DbPool;
use crate::models::ClientMessage;
use sqlx::Row;

pub struct PaginatedMessages {
    pub messages: Vec<ClientMessage>,
    pub min_id: Option<i64>,
    pub has_more: bool,
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
            "SELECT id, sender, group_id as room, content FROM group_messages 
            WHERE group_id = ? AND id < ? ORDER BY id DESC LIMIT ?",
        )
        .bind(room)
        .bind(before)
        .bind(actual_limit)
        .fetch_all(pool)
        .await?
    } else {
        sqlx::query(
            "SELECT id, sender, group_id as room, content FROM group_messages 
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
                msg_type: format!("message:{}", id), // 把 id 编码到 msg_type 里传回前端
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
