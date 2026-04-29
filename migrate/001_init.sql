-- =============================================================
-- rust_chat 数据库初始化迁移脚本
-- 版本：001
-- 描述：建立所有基础表结构
--       包含用户、会话、群聊消息、私聊消息、好友关系、
--       群组、群成员、离线消息、文件、统一已读游标
-- 执行方式：首次部署时由 sqlx::migrate! 自动运行，支持幂等（IF NOT EXISTS）
-- =============================================================


-- ── 用户表 ────────────────────────────────────────────────────
-- 存储注册用户的基本信息
CREATE TABLE IF NOT EXISTS users (
    id            INTEGER  PRIMARY KEY AUTOINCREMENT,   -- 自增主键
    username      TEXT     UNIQUE NOT NULL,             -- 用户名，全局唯一
    password_hash TEXT,                                 -- 密码哈希（argon2），禁止明文存储
    created_at    DATETIME DEFAULT CURRENT_TIMESTAMP    -- 注册时间，自动填充
);


-- ── 群聊消息表 ────────────────────────────────────────────────
-- 仅存储群聊消息，recalled = 1 表示该消息已被撤回
CREATE TABLE IF NOT EXISTS group_messages (
    id         INTEGER  PRIMARY KEY AUTOINCREMENT,
    group_id   TEXT     NOT NULL,                       -- 所属群组，关联 groups.group_id
    sender     TEXT     NOT NULL,                       -- 发送者用户名
    content    TEXT     NOT NULL,                       -- 消息正文
    recalled   INTEGER  NOT NULL DEFAULT 0,             -- 是否已撤回：0 正常 | 1 已撤回
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP
);

-- 按群组 + 消息 ID 的复合索引，支持高效分页拉取历史
CREATE INDEX IF NOT EXISTS idx_group_messages_group_id
    ON group_messages(group_id, id);


-- ── 私聊消息表 ────────────────────────────────────────────────
-- 仅存储私聊消息，recalled = 1 表示该消息已被撤回
CREATE TABLE IF NOT EXISTS private_messages (
    id         INTEGER  PRIMARY KEY AUTOINCREMENT,
    conv_id    TEXT     NOT NULL,                       -- 所属私聊会话，关联 conversations.conv_id
    sender     TEXT     NOT NULL,                       -- 发送者用户名
    content    TEXT     NOT NULL,                       -- 消息正文
    recalled   INTEGER  NOT NULL DEFAULT 0,             -- 是否已撤回：0 正常 | 1 已撤回
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP
);

-- 按会话 + 消息 ID 的复合索引，支持高效分页拉取历史
CREATE INDEX IF NOT EXISTS idx_private_messages_conv_id
    ON private_messages(conv_id, id);


-- ── 好友关系表 ────────────────────────────────────────────────
-- 记录用户之间的好友申请与关系状态
-- status 枚举：pending（待确认）| accepted（已成为好友）| rejected（已拒绝）
-- UNIQUE(from_user, to_user) 防止同一方向重复申请
CREATE TABLE IF NOT EXISTS friendships (
    id         INTEGER  PRIMARY KEY AUTOINCREMENT,
    from_user  TEXT     NOT NULL,                       -- 申请发起者用户名
    to_user    TEXT     NOT NULL,                       -- 申请接收者用户名
    status     TEXT     NOT NULL DEFAULT 'pending',     -- 关系状态
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    UNIQUE(from_user, to_user)                          -- 同一方向只允许一条记录
);


-- ── 群组表 ────────────────────────────────────────────────────
CREATE TABLE IF NOT EXISTS groups (
    id          INTEGER  PRIMARY KEY AUTOINCREMENT,
    group_id    TEXT     NOT NULL UNIQUE,               -- 群唯一标识（UUID）
    name        TEXT     NOT NULL,                      -- 群名称
    owner       TEXT     NOT NULL,                      -- 群主用户名
    avatar      TEXT     DEFAULT NULL,                  -- 群头像（base64 或 URL）
    notice      TEXT     DEFAULT NULL,                  -- 群公告
    created_at  DATETIME DEFAULT CURRENT_TIMESTAMP
);


-- ── 群成员表 ────────────────────────────────────────────────
-- role 枚举：'owner'（群主）| 'admin'（管理员）| 'member'（普通成员）
CREATE TABLE IF NOT EXISTS group_members (
    id          INTEGER  PRIMARY KEY AUTOINCREMENT,
    group_id    TEXT     NOT NULL,                      -- 关联 groups.group_id
    username    TEXT     NOT NULL,                      -- 成员用户名
    role        TEXT     NOT NULL DEFAULT 'member',     -- 成员角色
    joined_at   DATETIME DEFAULT CURRENT_TIMESTAMP,
    UNIQUE(group_id, username)                          -- 同一群内用户唯一
);


-- ── 文件表 ────────────────────────────────────────────────────
-- 存储上传文件的元数据，实际文件保存在磁盘 storage_path 指向的路径
CREATE TABLE IF NOT EXISTS files (
    id           TEXT     PRIMARY KEY,                  -- UUID，作为文件唯一标识
    filename     TEXT     NOT NULL,                     -- 原始文件名
    mime_type    TEXT     NOT NULL DEFAULT 'application/octet-stream', -- MIME 类型
    file_size    INTEGER  NOT NULL DEFAULT 0,           -- 文件大小（字节）
    storage_path TEXT     NOT NULL,                     -- 磁盘实际存储路径
    uploader     TEXT     NOT NULL,                     -- 上传人用户名
    created_at   DATETIME DEFAULT CURRENT_TIMESTAMP
);


-- ── 统一已读游标表 ────────────────────────────────────────────
-- 记录每个用户在每个会话中最后已读的消息 ID，用于计算未读数
-- session_type 枚举：'group'（群聊）| 'private'（私聊）
-- session_id：群聊时为 group_id，私聊时为 conv_id
-- 未读数计算：SELECT COUNT(*) FROM {messages} WHERE session_id = ? AND id > last_read_id AND sender != username
CREATE TABLE IF NOT EXISTS read_cursor (
    username     TEXT     NOT NULL,
    session_id   TEXT     NOT NULL,
    session_type TEXT     NOT NULL DEFAULT 'group',     -- 会话类型：group | private
    last_read_id INTEGER  NOT NULL DEFAULT 0,           -- 最后已读的消息 id
    updated_at   DATETIME DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (username, session_id)                  -- 每个用户在每个会话内唯一一条
);