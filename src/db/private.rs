use sqlx::Row;
use crate::db::{DbPool, PaginatedMessages};
use crate::models::{ClientMessage, ConversationItem};

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

pub async fn get_user_conversations(
    pool: &DbPool,
    username: &str,
) -> anyhow::Result<Vec<ConversationItem>> {
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

pub async fn get_private_history_paginated(
    pool: &DbPool,
    conv_id: &str,
    before_id: Option<i64>,
    limit: i64,
) -> anyhow::Result<PaginatedMessages> {
    let actual_limit = limit + 1;
    let rows = if let Some(before) = before_id {
        sqlx::query(
            "SELECT id, username, content FROM messages
             WHERE conversation_id = ? AND id < ? ORDER BY id DESC LIMIT ?",
        )
        .bind(conv_id)
        .bind(before)
        .bind(actual_limit)
        .fetch_all(pool)
        .await?
    } else {
        sqlx::query(
            "SELECT id, username, content FROM messages
             WHERE conversation_id = ? ORDER BY id DESC LIMIT ?",
        )
        .bind(conv_id)
        .bind(actual_limit)
        .fetch_all(pool)
        .await?
    };

    let has_more = (rows.len() as i64) > limit;
    let page_rows = if has_more { &rows[..(limit as usize)] } else { &rows[..] };

    let mut messages: Vec<ClientMessage> = page_rows
        .iter()
        .map(|row| {
            let id: i64 = row.try_get("id").unwrap_or(0);
            ClientMessage {
                username: row.try_get("username").unwrap_or_default(),
                room: "".into(),
                content: row.try_get("content").unwrap_or_default(),
                msg_type: format!("private:{}", id),
            }
        })
        .collect();
    messages.reverse();

    let min_id = if messages.is_empty() {
        None
    } else {
        Some(page_rows.iter().map(|r| {
            let id: i64 = r.try_get("id").unwrap_or(0);
            id
        }).min().unwrap_or(0))
    };

    Ok(PaginatedMessages { messages, min_id, has_more })
}