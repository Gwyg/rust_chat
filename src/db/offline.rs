use sqlx::Row;
use crate::db::DbPool;
use crate::models::ClientMessage;

pub async fn save_offline_message(
    pool: &DbPool,
    sender: &str,
    recipient: &str,
    content: &str,
) -> anyhow::Result<()> {
    sqlx::query(
        "INSERT INTO offline_messages (sender, recipient, content) VALUES (?, ?, ?)"
    )
    .bind(sender)
    .bind(recipient)
    .bind(content)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn get_offline_messages(
    pool: &DbPool,
    username: &str,
) -> anyhow::Result<Vec<ClientMessage>> {
    let rows = sqlx::query(
        "SELECT id, sender, content FROM offline_messages
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
            ClientMessage {
                msg_type: "private".into(),
                username: sender,
                room: "".into(),
                content,
            }
        })
        .collect();

    Ok(messages)
}

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