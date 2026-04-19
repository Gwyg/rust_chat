use crate::db::DbPool;
use crate::models::{GroupItem, GroupMemberItem};
use sqlx::Row;
use uuid::Uuid;

/// 创建群组，同时将群主写入 group_members
pub async fn create_group(
    pool: &DbPool,
    owner: &str,
    name: &str,
) -> anyhow::Result<String> {
    // 生成唯一群 ID
    let group_id = Uuid::new_v4().to_string();

    // 插入群组记录
    sqlx::query(
        "INSERT INTO groups (group_id, name, owner) VALUES (?, ?, ?)"
    )
    .bind(&group_id)
    .bind(name)
    .bind(owner)
    .execute(pool)
    .await?;

    // 将群主加入成员表，角色为 owner
    sqlx::query(
        "INSERT INTO group_members (group_id, username, role) VALUES (?, ?, 'owner')"
    )
    .bind(&group_id)
    .bind(owner)
    .execute(pool)
    .await?;

    Ok(group_id)
}

/// 查询用户所在的所有群组（含成员数和自身角色）
pub async fn get_user_groups(
    pool: &DbPool,
    username: &str,
) -> anyhow::Result<Vec<GroupItem>> {
    let rows = sqlx::query(
        "SELECT g.group_id, g.name, g.owner, g.avatar, g.notice,
                gm.role,
                (SELECT COUNT(*) FROM group_members gm2 WHERE gm2.group_id = g.group_id) AS member_count
         FROM groups g
         JOIN group_members gm ON g.group_id = gm.group_id
         WHERE gm.username = ?
         ORDER BY g.created_at DESC"
    )
    .bind(username)
    .fetch_all(pool)
    .await?;

    Ok(rows.iter().map(|r| GroupItem {
        group_id:     r.try_get("group_id").unwrap_or_default(),
        name:         r.try_get("name").unwrap_or_default(),
        owner:        r.try_get("owner").unwrap_or_default(),
        avatar:       r.try_get("avatar").ok().flatten(),
        notice:       r.try_get("notice").ok().flatten(),
        member_count: r.try_get("member_count").unwrap_or(0),
        role:         r.try_get("role").unwrap_or_default(),
    }).collect())
}

/// 查询群组详情（单条）
pub async fn get_group_detail(
    pool: &DbPool,
    group_id: &str,
    username: &str,
) -> anyhow::Result<Option<GroupItem>> {
    let row = sqlx::query(
        "SELECT g.group_id, g.name, g.owner, g.avatar, g.notice,
                gm.role,
                (SELECT COUNT(*) FROM group_members gm2 WHERE gm2.group_id = g.group_id) AS member_count
         FROM groups g
         JOIN group_members gm ON g.group_id = gm.group_id
         WHERE g.group_id = ? AND gm.username = ?"
    )
    .bind(group_id)
    .bind(username)
    .fetch_optional(pool)
    .await?;

    Ok(row.map(|r| GroupItem {
        group_id:     r.try_get("group_id").unwrap_or_default(),
        name:         r.try_get("name").unwrap_or_default(),
        owner:        r.try_get("owner").unwrap_or_default(),
        avatar:       r.try_get("avatar").ok().flatten(),
        notice:       r.try_get("notice").ok().flatten(),
        member_count: r.try_get("member_count").unwrap_or(0),
        role:         r.try_get("role").unwrap_or_default(),
    }))
}

/// 获取群成员列表
pub async fn get_group_members(
    pool: &DbPool,
    group_id: &str,
) -> anyhow::Result<Vec<GroupMemberItem>> {
    let rows = sqlx::query(
        "SELECT username, role, joined_at FROM group_members
         WHERE group_id = ?
         ORDER BY CASE role WHEN 'owner' THEN 0 WHEN 'admin' THEN 1 ELSE 2 END, joined_at ASC"
    )
    .bind(group_id)
    .fetch_all(pool)
    .await?;

    Ok(rows.iter().map(|r| GroupMemberItem {
        username:  r.try_get("username").unwrap_or_default(),
        role:      r.try_get("role").unwrap_or_default(),
        joined_at: r.try_get("joined_at").unwrap_or_default(),
    }).collect())
}

/// 邀请用户加入群组（仅群主/管理员可操作，权限校验在 handler 层）
pub async fn add_group_member(
    pool: &DbPool,
    group_id: &str,
    username: &str,
) -> anyhow::Result<bool> {
    let result = sqlx::query(
        "INSERT OR IGNORE INTO group_members (group_id, username, role) VALUES (?, ?, 'member')"
    )
    .bind(group_id)
    .bind(username)
    .execute(pool)
    .await?;

    // rows_affected == 0 说明已经是成员了（IGNORE 生效）
    Ok(result.rows_affected() > 0)
}

/// 踢出群成员（不能踢群主）
pub async fn remove_group_member(
    pool: &DbPool,
    group_id: &str,
    operator: &str,     // 操作者
    target: &str,       // 被踢者
) -> anyhow::Result<()> {
    // 获取操作者角色
    let op_role: Option<String> = sqlx::query_scalar(
        "SELECT role FROM group_members WHERE group_id = ? AND username = ?"
    )
    .bind(group_id)
    .bind(operator)
    .fetch_optional(pool)
    .await?;

    // 获取被踢者角色
    let target_role: Option<String> = sqlx::query_scalar(
        "SELECT role FROM group_members WHERE group_id = ? AND username = ?"
    )
    .bind(group_id)
    .bind(target)
    .fetch_optional(pool)
    .await?;

    match (op_role.as_deref(), target_role.as_deref()) {
        (Some("owner"), Some(r)) if r != "owner" => {}         // 群主可踢任何非群主
        (Some("admin"), Some("member")) => {}                   // 管理员只能踢普通成员
        _ => return Err(anyhow::anyhow!("权限不足或目标不可踢")),
    }

    sqlx::query(
        "DELETE FROM group_members WHERE group_id = ? AND username = ?"
    )
    .bind(group_id)
    .bind(target)
    .execute(pool)
    .await?;

    Ok(())
}

/// 更新群公告（仅群主/管理员）
pub async fn update_group_notice(
    pool: &DbPool,
    group_id: &str,
    operator: &str,
    notice: &str,
) -> anyhow::Result<()> {
    // 校验权限
    let role: Option<String> = sqlx::query_scalar(
        "SELECT role FROM group_members WHERE group_id = ? AND username = ?"
    )
    .bind(group_id)
    .bind(operator)
    .fetch_optional(pool)
    .await?;

    match role.as_deref() {
        Some("owner") | Some("admin") => {}
        _ => return Err(anyhow::anyhow!("权限不足，仅群主或管理员可修改公告")),
    }

    sqlx::query("UPDATE groups SET notice = ? WHERE group_id = ?")
        .bind(notice)
        .bind(group_id)
        .execute(pool)
        .await?;

    Ok(())
}

/// 更新群头像（仅群主）
pub async fn update_group_avatar(
    pool: &DbPool,
    group_id: &str,
    operator: &str,
    avatar: &str,
) -> anyhow::Result<()> {
    // 仅群主可改头像
    let role: Option<String> = sqlx::query_scalar(
        "SELECT role FROM group_members WHERE group_id = ? AND username = ?"
    )
    .bind(group_id)
    .bind(operator)
    .fetch_optional(pool)
    .await?;

    if role.as_deref() != Some("owner") {
        return Err(anyhow::anyhow!("权限不足，仅群主可修改群头像"));
    }

    sqlx::query("UPDATE groups SET avatar = ? WHERE group_id = ?")
        .bind(avatar)
        .bind(group_id)
        .execute(pool)
        .await?;

    Ok(())
}

/// 解散群组（仅群主），同时删除所有成员记录
pub async fn dissolve_group(
    pool: &DbPool,
    group_id: &str,
    operator: &str,
) -> anyhow::Result<()> {
    // 仅群主可解散
    let role: Option<String> = sqlx::query_scalar(
        "SELECT role FROM group_members WHERE group_id = ? AND username = ?"
    )
    .bind(group_id)
    .bind(operator)
    .fetch_optional(pool)
    .await?;

    if role.as_deref() != Some("owner") {
        return Err(anyhow::anyhow!("权限不足，仅群主可解散群组"));
    }

    sqlx::query("DELETE FROM group_members WHERE group_id = ?").bind(group_id).execute(pool).await?;
    sqlx::query("DELETE FROM groups WHERE group_id = ?").bind(group_id).execute(pool).await?;

    Ok(())
}

// ============================================================
// 群聊未读游标相关
// ============================================================

/// 更新用户在某个群内的已读游标
///
/// 使用 INSERT OR REPLACE 保证幂等：如果记录已存在则更新，不存在则插入
pub async fn update_group_read_cursor(
    pool: &DbPool,
    username: &str,
    group_id: &str,
    message_id: i64,
) -> anyhow::Result<()> {
    sqlx::query(
        "INSERT INTO group_read_cursor (username, group_id, last_read_id, updated_at)
         VALUES (?, ?, ?, CURRENT_TIMESTAMP)
         ON CONFLICT(username, group_id)
         DO UPDATE SET last_read_id = excluded.last_read_id, updated_at = CURRENT_TIMESTAMP"
    )
    .bind(username)
    .bind(group_id)
    .bind(message_id)
    .execute(pool)
    .await?;
    Ok(())
}

/// 查询用户在某个群的未读消息数
///
/// 计算逻辑：group_messages 中 id > last_read_id 的消息数量
/// 如果用户没有游标记录（从未标记已读），返回 0
pub async fn get_group_unread_count(
    pool: &DbPool,
    username: &str,
    group_id: &str,
) -> anyhow::Result<i64> {
    let cursor: Option<i64> = sqlx::query_scalar(
        "SELECT last_read_id FROM group_read_cursor
         WHERE username = ? AND group_id = ?"
    )
    .bind(username)
    .bind(group_id)
    .fetch_optional(pool)
    .await?;

    match cursor {
        Some(last_read_id) => {
            let count: i64 = sqlx::query_scalar(
                "SELECT COUNT(*) FROM group_messages
                 WHERE group_id = ? AND id > ?"
            )
            .bind(group_id)
            .bind(last_read_id)
            .fetch_one(pool)
            .await?;
            Ok(count)
        }
        None => Ok(0), // 没有游标记录，说明从未标记已读，返回 0
    }
}
