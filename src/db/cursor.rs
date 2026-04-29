use crate::db::DbPool;
use sqlx::Row;

/// 更新已读游标到指定消息 ID
///
/// - session_type: "group" | "private"
/// - session_id:   群聊为 group_id，私聊为 conv_id
/// - last_read_id: 当前已读到的最新消息 id
pub async fn update_read_cursor(
    pool: &DbPool,
    username: &str,
    session_type: &str,
    session_id: &str,
    last_read_id: i64,
) -> anyhow::Result<()> {
    sqlx::query(
        "INSERT INTO read_cursor (username, session_id, session_type, last_read_id, updated_at)
         VALUES (?, ?, ?, ?, CURRENT_TIMESTAMP)
         ON CONFLICT(username, session_id)
         DO UPDATE SET last_read_id = excluded.last_read_id,
                       updated_at   = CURRENT_TIMESTAMP"
    )
    .bind(username)
    .bind(session_id)
    .bind(session_type)
    .bind(last_read_id)
    .execute(pool)
    .await?;
    Ok(())
}

/// 将游标推到当前会话最新消息（打开会话时调用）
///
/// 内部先查 MAX(id)，再调 update_read_cursor，保证幂等
pub async fn mark_session_read(
    pool: &DbPool,
    username: &str,
    session_type: &str,
    session_id: &str,
) -> anyhow::Result<()> {
    let table = match session_type {
        "group"   => "group_messages",
        "private" => "private_messages",
        _         => return Err(anyhow::anyhow!("无效的 session_type")),
    };

    let id_col = match session_type {
        "group"   => "group_id",
        "private" => "conv_id",
        _         => unreachable!(),
    };

    let max_id: Option<i64> = sqlx::query(
        &format!("SELECT MAX(id) AS max_id FROM {} WHERE {} = ?", table, id_col)
    )
    .bind(session_id)
    .fetch_optional(pool)
    .await?
    .and_then(|r| r.try_get("max_id").ok());

    // 没有消息时不写游标
    if let Some(id) = max_id {
        update_read_cursor(pool, username, session_type, session_id, id).await?;
    }
    Ok(())
}

/// 查询某个会话的未读消息数
///
/// 未读 = 消息表中 id > last_read_id 且不是自己发的
pub async fn get_unread_count(
    pool: &DbPool,
    username: &str,
    session_type: &str,
    session_id: &str,
) -> anyhow::Result<i64> {
    let cursor: Option<i64> = sqlx::query(
        "SELECT last_read_id FROM read_cursor
         WHERE username = ? AND session_id = ? AND session_type = ?"
    )
    .bind(username)
    .bind(session_id)
    .bind(session_type)
    .fetch_optional(pool)
    .await?
    .and_then(|r| r.try_get("last_read_id").ok());

    let last_read_id = match cursor {
        Some(id) => id,
        None => return Ok(0), // 从未打开过，不计未读（避免全量历史都算未读）
    };

    let (table, id_col, sender_col) = match session_type {
        "group"   => ("group_messages",   "group_id", "sender"),
        "private" => ("private_messages", "conv_id",  "sender"),
        _         => return Err(anyhow::anyhow!("无效的 session_type")),
    };

    let count: i64 = sqlx::query(
        &format!(
            "SELECT COUNT(*) AS cnt FROM {} WHERE {} = ? AND id > ? AND {} != ?",
            table, id_col, sender_col
        )
    )
    .bind(session_id)
    .bind(last_read_id)
    .bind(username)   // 不计自己发的消息
    .fetch_one(pool)
    .await?
    .try_get("cnt")
    .unwrap_or(0);

    Ok(count)
}

/// 拉取某个会话中用户未读的所有消息（id > last_read_id，不含自己发的）
/// 用于切换会话时主动推送未读内容给用户
pub async fn get_unread_messages(
    pool: &DbPool,
    username: &str,
    session_type: &str,
    session_id: &str,
) -> anyhow::Result<Vec<crate::models::ClientMessage>> {
    use sqlx::Row;

    let cursor: Option<i64> = sqlx::query(
        "SELECT last_read_id FROM read_cursor
         WHERE username = ? AND session_id = ? AND session_type = ?"
    )
    .bind(username)
    .bind(session_id)
    .bind(session_type)
    .fetch_optional(pool)
    .await?
    .and_then(|r| r.try_get("last_read_id").ok());

    let last_read_id = match cursor {
        Some(id) => id,
        None => return Ok(vec![]), // 没有游标，说明从未打开过，不推
    };

    let (table, id_col, type_label) = match session_type {
        "group"   => ("group_messages",   "group_id", "message"),
        "private" => ("private_messages", "conv_id",  "private"),
        _         => return Err(anyhow::anyhow!("无效的 session_type")),
    };

    let rows = sqlx::query(
        &format!(
            "SELECT id, sender, content, recalled FROM {}
             WHERE {} = ? AND id > ? AND sender != ?
             ORDER BY id ASC",
            table, id_col
        )
    )
    .bind(session_id)
    .bind(last_read_id)
    .bind(username)
    .fetch_all(pool)
    .await?;

    let messages = rows.iter().map(|row| {
        let id: i64 = row.try_get("id").unwrap_or(0);
        crate::models::ClientMessage {
            msg_type: type_label.into(),
            username: row.try_get("sender").unwrap_or_default(),
            room: session_id.into(),
            content: row.try_get("content").unwrap_or_default(),
            recalled: row.try_get("recalled").unwrap_or(false),
            message_id: Some(id),
            ..Default::default()
        }
    }).collect();

    Ok(messages)
}