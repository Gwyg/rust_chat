-- =============================================================
-- rust_chat 数据库初始化迁移脚本
-- 描述：一次性建立所有基础表结构
--       包含用户、群聊消息、私聊消息、会话、好友关系、
--       群组、群成员、离线消息、群聊未读游标
-- 执行方式：首次部署时运行一次，支持幂等（IF NOT EXISTS）
-- =============================================================


-- 用户表：存储注册用户的基本信息
CREATE TABLE IF NOT EXISTS users (
    id            INTEGER  PRIMARY KEY AUTOINCREMENT,          -- 自增主键
    username      TEXT     UNIQUE NOT NULL,                    -- 用户名，全局唯一
    password_hash TEXT,                                        -- 密码哈希（argon2），禁止明文存储
    created_at    DATETIME DEFAULT CURRENT_TIMESTAMP           -- 注册时间，自动填充
);


-- 会话表：管理私聊会话，每对用户对应一条唯一记录
-- conv_id 由业务层生成（将双方用户名按字典序排序后以 _ 拼接：user_a_user_b）
CREATE TABLE IF NOT EXISTS conversations (
    id         INTEGER  PRIMARY KEY AUTOINCREMENT,             -- 自增主键
    conv_id    TEXT     NOT NULL UNIQUE,                       -- 唯一会话标识
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP              -- 会话创建时间
);


-- 群聊消息表：仅存储群聊消息
CREATE TABLE IF NOT EXISTS group_messages (
    id         INTEGER  PRIMARY KEY AUTOINCREMENT,             -- 自增主键
    group_id   TEXT     NOT NULL,                              -- 所属群组，关联 groups.group_id
    sender     TEXT     NOT NULL,                              -- 发送者用户名
    content    TEXT     NOT NULL,                              -- 消息正文
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP              -- 发送时间
);

-- 按群组 + 消息 ID 的复合索引，支持高效分页拉取历史
CREATE INDEX IF NOT EXISTS idx_group_messages_group_id
    ON group_messages(group_id, id);


-- 私聊消息表：仅存储私聊消息
CREATE TABLE IF NOT EXISTS private_messages (
    id         INTEGER  PRIMARY KEY AUTOINCREMENT,             -- 自增主键
    conv_id    TEXT     NOT NULL,                              -- 所属私聊会话，关联 conversations.conv_id
    sender     TEXT     NOT NULL,                              -- 发送者用户名
    content    TEXT     NOT NULL,                              -- 消息正文
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP              -- 发送时间
);

-- 按会话 + 消息 ID 的复合索引，支持高效分页拉取历史
CREATE INDEX IF NOT EXISTS idx_private_messages_conv_id
    ON private_messages(conv_id, id);


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


-- 群组表：用户自建群组
CREATE TABLE IF NOT EXISTS groups (
    id          INTEGER  PRIMARY KEY AUTOINCREMENT,            -- 自增主键
    group_id    TEXT     NOT NULL UNIQUE,                      -- 群唯一标识（UUID 或 owner_时间戳）
    name        TEXT     NOT NULL,                             -- 群名称
    owner       TEXT     NOT NULL,                             -- 群主用户名
    avatar      TEXT     DEFAULT NULL,                         -- 群头像（base64 或 URL）
    notice      TEXT     DEFAULT NULL,                         -- 群公告
    created_at  DATETIME DEFAULT CURRENT_TIMESTAMP             -- 创建时间
);


-- 群成员表：记录群与成员的关系
-- role 枚举：'owner'（群主）| 'admin'（管理员）| 'member'（普通成员）
CREATE TABLE IF NOT EXISTS group_members (
    id          INTEGER  PRIMARY KEY AUTOINCREMENT,            -- 自增主键
    group_id    TEXT     NOT NULL,                             -- 关联 groups.group_id
    username    TEXT     NOT NULL,                             -- 成员用户名
    role        TEXT     NOT NULL DEFAULT 'member',            -- 成员角色
    joined_at   DATETIME DEFAULT CURRENT_TIMESTAMP,            -- 加入时间
    UNIQUE(group_id, username)                                 -- 同一群内用户唯一
);


-- 离线消息表：统一存储私聊和群聊的离线消息
-- type 枚举：'private'（私聊）| 'group'（群聊）
-- source_id：私聊时为 conv_id，群聊时为 group_id；前端据此跳转对应会话
CREATE TABLE IF NOT EXISTS offline_messages (
    id         INTEGER  PRIMARY KEY AUTOINCREMENT,             -- 自增主键
    recipient  TEXT     NOT NULL,                              -- 接收者用户名
    sender     TEXT     NOT NULL,                              -- 发送者用户名
    content    TEXT     NOT NULL,                              -- 消息正文
    type       TEXT     NOT NULL DEFAULT 'private',            -- 消息来源类型：private | group
    source_id  TEXT     NOT NULL,                              -- 来源 ID（conv_id 或 group_id）
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP              -- 入库时间
);

-- 按收件人 + ID 的复合索引，批量拉取离线消息
CREATE INDEX IF NOT EXISTS idx_offline_recipient
    ON offline_messages(recipient, id);


-- 群聊未读游标表：记录每个用户在每个群内最后读到的消息 ID
-- 查询未读数：SELECT count(*) FROM group_messages
--             WHERE group_id = ? AND id > last_read_id
CREATE TABLE IF NOT EXISTS group_read_cursor (
    username     TEXT     NOT NULL,                            -- 用户名
    group_id     TEXT     NOT NULL,                            -- 群组 ID，关联 groups.group_id
    last_read_id INTEGER  NOT NULL DEFAULT 0,                  -- 最后已读的 group_messages.id
    updated_at   DATETIME DEFAULT CURRENT_TIMESTAMP,           -- 游标更新时间
    PRIMARY KEY (username, group_id)                           -- 每个用户在每个群内唯一一条
);

-- 文件表：存储上传文件的元数据
CREATE TABLE IF NOT EXISTS files (
    id           TEXT     PRIMARY KEY,                     -- UUID
    filename     TEXT     NOT NULL,                        -- 原始文件名
    mime_type    TEXT     NOT NULL DEFAULT 'application/octet-stream',
    file_size    INTEGER  NOT NULL DEFAULT 0,              -- 字节数
    storage_path TEXT     NOT NULL,                        -- 磁盘实际路径
    uploader     TEXT     NOT NULL,                        -- 上传人用户名
    created_at   DATETIME DEFAULT CURRENT_TIMESTAMP
);
