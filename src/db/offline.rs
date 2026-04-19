use sqlx::Row;
use crate::db::DbPool;
use crate::models::ClientMessage;

pub async fn save_offline_message(
    pool: &DbPool,
    sender: &str,
    recipient: &str,
    content: &str,
    msg_type: &str,
    source_id: &str,
) -> anyhow::Result<()> {
    sqlx::query(
        "INSERT INTO offline_messages (recipient, sender, content, type, source_id) VALUES (?, ?, ?, ?, ?)"
    )
    .bind(recipient)
    .bind(sender)
    .bind(content)
    .bind(msg_type)
    .bind(source_id)
    .execute(pool)
    .await?;
    Ok(())
}

/// 获取未读摘要：每个 source_id 的未读数量
pub async fn get_unread_summary(
    pool: &DbPool,
    username: &str,
) -> anyhow::Result<Vec<(String, String, i64)>> {
    // 返回 (type, source_id, count)
    let rows = sqlx::query(
        "SELECT type, source_id, COUNT(*) as count
         FROM offline_messages
         WHERE recipient = ?
         GROUP BY type, source_id
         ORDER BY MIN(id) ASC"
    )
    .bind(username)
    .fetch_all(pool)
    .await?;

    let result = rows
        .into_iter()
        .map(|row| {
            let msg_type: String = row.try_get("type").unwrap_or_default();
            let source_id: String = row.try_get("source_id").unwrap_or_default();
            let count: i64 = row.try_get("count").unwrap_or(0);
            (msg_type, source_id, count)
        })
        .collect();

    Ok(result)
}

/// 获取指定会话的离线消息
pub async fn get_offline_messages_by_source(
    pool: &DbPool,
    username: &str,
    source_id: &str,
) -> anyhow::Result<Vec<ClientMessage>> {
    let rows = sqlx::query(
        "SELECT sender, content, type, source_id
         FROM offline_messages
         WHERE recipient = ? AND source_id = ?
         ORDER BY id ASC"
    )
    .bind(username)
    .bind(source_id)
    .fetch_all(pool)
    .await?;

    let messages = rows
        .into_iter()
        .map(|row| {
            let sender: String = row.try_get("sender").unwrap_or_default();
            let content: String = row.try_get("content").unwrap_or_default();
            let msg_type: String = row.try_get("type").unwrap_or_else(|_| "private".into());
            let source_id: String = row.try_get("source_id").unwrap_or_default();
            ClientMessage {
                msg_type,
                username: sender,
                room: source_id,
                content,
                ..Default::default()
            }
        })
        .collect();

    Ok(messages)
}

/// 清除指定会话的离线消息
pub async fn clear_offline_messages_by_source(
    pool: &DbPool,
    username: &str,
    source_id: &str,
) -> anyhow::Result<()> {
    sqlx::query(
        "DELETE FROM offline_messages WHERE recipient = ? AND source_id = ?"
    )
    .bind(username)
    .bind(source_id)
    .execute(pool)
    .await?;
    Ok(())
}
