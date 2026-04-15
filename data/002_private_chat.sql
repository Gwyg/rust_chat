-- 私聊会话表
CREATE TABLE IF NOT EXISTS conversations (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    conv_id     TEXT    NOT NULL UNIQUE,          -- "userA_userB"（排序后拼接）
    type        TEXT    NOT NULL DEFAULT 'private', -- 'private' | 'group'
    created_at  TEXT    NOT NULL DEFAULT (datetime('now'))
);

-- 给 messages 表加 conversation_id 列
ALTER TABLE messages ADD COLUMN conversation_id TEXT DEFAULT NULL;