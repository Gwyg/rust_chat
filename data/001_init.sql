-- =============================================================
-- rust_chat 数据库初始化迁移脚本
-- 描述：一次性建立所有基础表结构，包含用户、消息、会话、好友关系
-- 执行方式：首次部署时运行一次，支持幂等（IF NOT EXISTS）
-- =============================================================


-- 用户表：存储注册用户的基本信息
CREATE TABLE IF NOT EXISTS users (
    id            INTEGER  PRIMARY KEY AUTOINCREMENT,          -- 自增主键
    username      TEXT     UNIQUE NOT NULL,                    -- 用户名，全局唯一
    password_hash TEXT,                                        -- 密码哈希（bcrypt/argon2），禁止明文存储
    created_at    DATETIME DEFAULT CURRENT_TIMESTAMP           -- 注册时间，自动填充
);


-- 消息表：存储群聊和私聊的所有消息记录
CREATE TABLE IF NOT EXISTS messages (
    id              INTEGER  PRIMARY KEY AUTOINCREMENT,        -- 自增主键
    username        TEXT     NOT NULL,                         -- 发送者用户名
    room            TEXT     NOT NULL,                         -- 所属房间名（群聊）
    content         TEXT     NOT NULL,                         -- 消息正文
    conversation_id TEXT     DEFAULT NULL,                     -- 私聊会话 ID，关联 conversations.conv_id；群聊时为 NULL
    created_at      DATETIME DEFAULT CURRENT_TIMESTAMP         -- 消息发送时间
);


-- 会话表：管理私聊会话，每对用户对应一条唯一记录
-- conv_id 由业务层生成（如将双方用户名排序后拼接：user_a:user_b）
CREATE TABLE IF NOT EXISTS conversations (
    id         INTEGER  PRIMARY KEY AUTOINCREMENT,             -- 自增主键
    conv_id    TEXT     NOT NULL UNIQUE,                       -- 唯一会话标识
    type       TEXT     NOT NULL DEFAULT 'private',            -- 会话类型：private（私聊），预留 group 扩展
    created_at TEXT     NOT NULL DEFAULT (datetime('now'))     -- 会话创建时间
);


-- 好友关系表：记录用户之间的好友申请与关系状态
-- status 枚举：pending（待确认）| accepted（已成为好友）| rejected（已拒绝）
-- UNIQUE(from_user, to_user) 防止同一方向重复申请
CREATE TABLE IF NOT EXISTS friendships (
    id         INTEGER  PRIMARY KEY AUTOINCREMENT,             -- 自增主键
    from_user  TEXT     NOT NULL,                              -- 申请发起者用户名
    to_user    TEXT     NOT NULL,                              -- 申请接收者用户名
    status     TEXT     NOT NULL DEFAULT 'pending',            -- 关系状态
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,             -- 申请时间
    UNIQUE(from_user, to_user)                                 -- 同一方向只允许一条记录
);
-- 群组表：用户自建群组（区别于 main.rs 里的硬编码系统群）
CREATE TABLE IF NOT EXISTS groups (
    id          INTEGER  PRIMARY KEY AUTOINCREMENT,
    group_id    TEXT     NOT NULL UNIQUE,              -- 群唯一标识（UUID 或 owner_时间戳）
    name        TEXT     NOT NULL,                     -- 群名称
    owner       TEXT     NOT NULL,                     -- 群主用户名
    avatar      TEXT     DEFAULT NULL,                 -- 群头像（base64 或 URL）
    notice      TEXT     DEFAULT NULL,                 -- 群公告
    created_at  DATETIME DEFAULT CURRENT_TIMESTAMP     -- 创建时间
);

-- 群成员表：记录群与成员的关系
-- role 枚举：'owner'（群主）| 'admin'（管理员）| 'member'（普通成员）
CREATE TABLE IF NOT EXISTS group_members (
    id          INTEGER  PRIMARY KEY AUTOINCREMENT,
    group_id    TEXT     NOT NULL,                     -- 关联 groups.group_id
    username    TEXT     NOT NULL,                     -- 成员用户名
    role        TEXT     NOT NULL DEFAULT 'member',    -- 成员角色
    joined_at   DATETIME DEFAULT CURRENT_TIMESTAMP,    -- 加入时间
    UNIQUE(group_id, username)                         -- 同一群内用户唯一
);

-- =============================================================
-- 离线消息表
-- 用于存储用户不在线时收到的私聊消息
-- =============================================================
CREATE TABLE IF NOT EXISTS offline_messages (
    id         INTEGER PRIMARY KEY AUTOINCREMENT,
    sender     TEXT NOT NULL,
    recipient  TEXT NOT NULL,
    content    TEXT NOT NULL,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP
);

-- 按收件人查询的索引
CREATE INDEX IF NOT EXISTS idx_offline_recipient
    ON offline_messages(recipient, id);