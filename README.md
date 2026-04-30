# rust_chat

基于 Rust + Axum + Tokio 实现的全栈即时通讯系统，支持群聊、私聊、好友管理、群组管理、文件收发与离线消息推送。

## 技术栈

| 组件 | 版本 |
|------|------|
| Rust | edition 2024 |
| Axum | 0.7（含 WebSocket、Multipart） |
| Tokio | 1.48（full features） |
| SQLx | 0.7（SQLite，异步） |
| JWT | jsonwebtoken 9 |
| 密码哈希 | Argon2 |
| 序列化 | serde / serde_json |
| 日志 | tracing / tracing-subscriber |

## 功能特性

- **群聊**：多人实时消息广播，支持历史记录分页加载、消息撤回（2 分钟内）
- **私聊**：一对一消息推送，支持历史记录与离线消息
- **好友管理**：申请、接受、删除好友
- **群组管理**：创建/解散群组，添加/移除成员，修改群公告和头像
- **文件收发**：上传（≤100MB）、下载，群聊和私聊均支持
- **离线消息**：基于读游标（read_cursor）设计，上线推送未读摘要角标，切换会话时按游标拉取未读消息并自动标记已读
- **心跳检测**：每 30 秒服务端主动 Ping，断线自动清理在线状态
- **注册/登录**：Argon2 密码哈希，JWT 鉴权，Axum middleware 统一拦截未认证请求

## 并发设计

- **群聊**：`broadcast::channel` 实现一对多广播，每个群一个 Sender，订阅者各持一个 Receiver
- **私聊**：`mpsc::channel` 实现点对点推送，每个在线用户持有一个 Sender，存于 `private_rooms`
- **主循环**：`tokio::select!` 多路复用心跳、客户端消息、群聊消息、私聊消息四路，零阻塞

## 项目结构

```
src/
├── main.rs                  # 启动入口，DB 初始化、路由绑定
├── state.rs                 # AppState（group_rooms / private_rooms / online / db）
├── auth/
│   ├── middleware.rs        # Axum JWT 鉴权中间件
│   ├── token.rs             # JWT 签发与验证
│   └── password.rs          # Argon2 密码哈希
├── handlers/
│   ├── ws.rs                # WebSocket 升级入口
│   ├── auth.rs              # 登录、注册页面及 API
│   ├── chat.rs              # 群聊历史分页接口
│   ├── friends.rs           # 好友管理接口
│   ├── group.rs             # 群组管理接口
│   ├── file.rs              # 文件上传/下载接口
│   └── unread.rs            # 未读数查询与标记接口
├── ws/
│   ├── mod.rs               # WebSocket 主循环（handler_socket）
│   └── message.rs           # 消息分发处理（群聊/私聊/文件/撤回）
├── db/
│   ├── message.rs           # 群聊消息存取、撤回、分页
│   ├── private.rs           # 私聊消息存取、撤回
│   ├── user.rs              # 用户注册、查询
│   ├── friend.rs            # 好友关系增删查
│   ├── group.rs             # 群组与成员管理
│   ├── file.rs              # 文件元数据存取
│   └── cursor.rs            # 已读游标读写
└── models/
    ├── ws.rs                # ClientMessage / ServerMessage / UnreadSummary
    ├── api.rs               # HTTP 请求/响应结构体
    └── conversation.rs      # 会话模型
migrate/
└── 001_init.sql             # 数据库初始化迁移脚本
static/                      # 前端静态文件
```

## 快速开始

### 环境要求

- Rust 1.80+（推荐使用 rustup 安装）
- 无需额外安装数据库，SQLite 内嵌

### 启动

```bash
# 克隆项目
git clone https://github.com/Gwyg/rust_chat.git
cd rust_chat

# 编译并启动（首次编译较慢）
cargo run

# 服务默认监听
# http://127.0.0.1:3000
```

首次启动时 SQLx 会自动执行 `migrate/001_init.sql` 完成数据库初始化，数据库文件为 `chat.db`。

### 访问

浏览器打开 `http://127.0.0.1:3000`，注册账号后即可使用。

## API 概览

| 方法 | 路径 | 说明 |
|------|------|------|
| POST | `/api/register` | 注册 |
| POST | `/api/login` | 登录，返回 JWT |
| GET | `/ws?token=<jwt>` | WebSocket 连接 |
| GET | `/api/users` | 用户列表 |
| GET/POST | `/api/groups` | 群组列表 / 创建群组 |
| DELETE | `/api/groups/:group_id` | 解散群组 |
| POST | `/api/groups/members/add` | 添加群成员 |
| POST | `/api/groups/members/remove` | 移除群成员 |
| GET | `/api/rooms/:room/history` | 群聊历史分页 |
| GET | `/api/private/:target/history` | 私聊历史 |
| GET/POST | `/api/friends` | 好友列表 / 发送申请 |
| POST | `/api/friends/accept` | 接受好友申请 |
| DELETE | `/api/friends/:target` | 删除好友 |
| POST | `/api/upload` | 上传文件 |
| GET | `/api/download/:file_id` | 下载文件 |
| POST | `/api/messages/:message_id/recall` | 撤回消息 |
| GET | `/api/unread` | 查询未读数 |
| POST | `/api/read` | 标记已读 |

## WebSocket 消息格式

连接地址：`ws://127.0.0.1:3000/ws?token=<jwt>`

### 客户端 → 服务端

```json
// 切换到群聊
{ "msg_type": "switch", "room": "group_id", "username": "", "content": "" }

// 切换到私聊
{ "msg_type": "switch_private", "room": "target_username", "username": "", "content": "" }

// 发送群聊消息
{ "msg_type": "message", "room": "group_id", "username": "", "content": "消息内容" }

// 发送私聊消息
{ "msg_type": "private", "room": "target_username", "username": "", "content": "消息内容" }

// 发送文件
{ "msg_type": "file", "room": "group_id或target_username", "file_id": "uuid", "filename": "文件名", "mime_type": "image/png", "content": "" }
```

### 服务端 → 客户端

```json
// 普通消息
{ "type": "message", "username": "发送者", "content": "内容", "message_id": 123 }

// 未读摘要（上线时推送）
{ "msg_type": "unread_summary", "items": [{ "type": "group", "id": "group_id", "count": 5 }] }

// 消息撤回
{ "type": "message", "username": "发送者", "content": "", "message_id": 123, "recalled": true }

// 错误信息
{ "type": "error", "username": "", "content": "错误描述" }
```

## License

MIT
