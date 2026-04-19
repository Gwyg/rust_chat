use crate::db::{DbPool, PaginatedMessages};
use crate::models::{ClientMessage, ConversationItem};
use sqlx::Row;

pub async fn save_private_message(
    pool: &DbPool,
    username: &str,
    conv_id: &str,
    content: &str,
) -> anyhow::Result<()> {
    sqlx::query("INSERT INTO private_messages (conv_id, sender, content) VALUES (?, ?, ?)")
        .bind(conv_id)
        .bind(username)
        .bind(content)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn get_user_conversations(
    pool: &DbPool,
    username: &str,
) -> anyhow::Result<Vec<ConversationItem>> {
    let rows = sqlx::query(
        "SELECT c.conv_id,
                pm.content as last_content,
                pm.id as last_id
            FROM conversations c
            JOIN private_messages pm ON c.conv_id = pm.conv_id
            WHERE c.conv_id LIKE ? 
            AND pm.id = (SELECT MAX(id) FROM private_messages WHERE conv_id = c.conv_id)",
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
        "SELECT group_id as conv_id, content as last_content, id as last_id
            FROM group_messages
            WHERE id = (SELECT MAX(id) FROM group_messages WHERE group_id = gm.group_id)",
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
            "SELECT id, sender, content FROM private_messages 
            WHERE conv_id = ? AND id < ? ORDER BY id DESC LIMIT ?",
        )
        .bind(conv_id)
        .bind(before)
        .bind(actual_limit)
        .fetch_all(pool)
        .await?
    } else {
        sqlx::query(
            "SELECT id, sender, content FROM private_messages 
            WHERE conv_id = ? ORDER BY id DESC LIMIT ?",
        )
        .bind(conv_id)
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
                room: "".into(),
                content: row.try_get("content").unwrap_or_default(),
                msg_type: format!("private:{}", id),
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
