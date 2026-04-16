-- 好友关系表
-- status: 'pending'（待接受）| 'accepted'（已接受）| 'rejected'（已拒绝）
CREATE TABLE IF NOT EXISTS friendships (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    from_user   TEXT NOT NULL,
    to_user     TEXT NOT NULL,
    status      TEXT NOT NULL DEFAULT 'pending',
    created_at  DATETIME DEFAULT CURRENT_TIMESTAMP,
    UNIQUE(from_user, to_user)
);