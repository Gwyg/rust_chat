use sqlx::Row;
use crate::db::DbPool;
use crate::models::FriendItem;

pub async fn send_friend_request(
    pool: &DbPool,
    from: &str,
    to: &str,
) -> anyhow::Result<()> {
    sqlx::query(
        "INSERT OR IGNORE INTO friendships (from_user, to_user, status) VALUES (?, ?, 'pending')"
    )
    .bind(from)
    .bind(to)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn accept_friend(
    pool: &DbPool,
    from: &str,
    to: &str,
) -> anyhow::Result<()> {
    sqlx::query(
        "UPDATE friendships SET status = 'accepted' 
         WHERE from_user = ? AND to_user = ? AND status = 'pending'"
    )
    .bind(from)
    .bind(to)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn delete_friend(
    pool: &DbPool,
    user_a: &str,
    user_b: &str,
) -> anyhow::Result<()> {
    sqlx::query(
        "DELETE FROM friendships WHERE (from_user = ? AND to_user = ?) OR (from_user = ? AND to_user = ?)"
    )
    .bind(user_a).bind(user_b)
    .bind(user_b).bind(user_a)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn get_friends(
    pool: &DbPool,
    username: &str,
) -> anyhow::Result<Vec<FriendItem>> {
    let sent = sqlx::query(
        "SELECT to_user as other, status FROM friendships WHERE from_user = ?"
    )
    .bind(username)
    .fetch_all(pool)
    .await?;

    let recv = sqlx::query(
        "SELECT from_user as other, status FROM friendships WHERE to_user = ?"
    )
    .bind(username)
    .fetch_all(pool)
    .await?;

    let mut items = vec![];

    for row in sent {
        let other: String = row.try_get("other").unwrap_or_default();
        let status: String = row.try_get("status").unwrap_or_default();
        items.push(FriendItem {
            username: other,
            status: if status == "accepted" {
                "accepted".into()
            } else {
                "pending_send".into()
            },
        });
    }

    for row in recv {
        let other: String = row.try_get("other").unwrap_or_default();
        let status: String = row.try_get("status").unwrap_or_default();
        if status == "accepted" {
            if !items.iter().any(|i| i.username == other) {
                items.push(FriendItem {
                    username: other,
                    status: "accepted".into(),
                });
            }
        } else {
            items.push(FriendItem {
                username: other,
                status: "pending_recv".into(),
            });
        }
    }

    Ok(items)
}