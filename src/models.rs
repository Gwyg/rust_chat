use serde::{Deserialize, Serialize};

// === WebSocket 消息 ===

#[derive(Deserialize, Serialize, Clone)]
pub struct ClientMessage {
    pub msg_type: String,
    pub username: String,
    pub room: String,
    pub content: String,
}

#[derive(Serialize, Clone)]
pub struct ServerMessage {
    #[serde(rename = "type")]
    pub msg_type: String,
    pub username: String,
    pub content: String,
}

// === API 请求/响应 ===

#[derive(Deserialize)]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
}

#[derive(Serialize)]
pub struct LoginResponse {
    pub token: String,
}

#[derive(Deserialize)]
pub struct RegisterRequest {
    pub username: String,
    pub password: String,
}

#[derive(Serialize)]
pub struct RegisterResponse {
    pub token: String,
}

// === 业务模型 ===

#[derive(Serialize)]
pub struct ConversationItem {
    pub conv_id: String,
    pub conv_type: String,
    pub name: String,
    pub last_content: Option<String>,
    pub last_time: Option<String>,
}

#[derive(Serialize)]
pub struct FriendItem {
    pub username: String,
    pub status: String,   // "accepted" | "pending_send" | "pending_recv"
}

#[derive(Deserialize)]
pub struct FriendRequest {
    pub target: String,   // 对方用户名
}