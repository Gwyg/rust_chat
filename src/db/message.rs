use sqlx::Row;
use crate::db::DbPool;
use crate::models::ClientMessage;

pub async fn save_message(
    pool: &DbPool,
    username: &str,
    room: &str,
    content: &str,
) -> anyhow::Result<()> {
    sqlx::query("INSERT INTO messages (username, room, content, conversation_id) VALUES (?, ?, NULL)")
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