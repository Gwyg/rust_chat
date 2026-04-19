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
/// 获取用户的所有离线消息
pub async fn get_offline_messages(
    pool: &DbPool,
    username: &str,
) -> anyhow::Result<Vec<ClientMessage>> {
    let rows = sqlx::query(
        "SELECT id, sender, content, type, source_id FROM offline_messages
         WHERE recipient = ? ORDER BY id ASC"
    )
    .bind(username)
    .fetch_all(pool)
    .await?;

    let messages: Vec<ClientMessage> = rows
        .into_iter()
        .map(|row| {
            let _: i64 = row.try_get("id").unwrap_or(0);
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

/// 清除用户的所有离线消息
pub async fn clear_offline_messages(
    pool: &DbPool,
    username: &str,
) -> anyhow::Result<()> {
    sqlx::query("DELETE FROM offline_messages WHERE recipient = ?")
        .bind(username)
        .execute(pool)
        .await?;
    Ok(())
}
