use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize, Clone)]
pub struct ClientMessage {
    pub msg_type: String,       // 新增: "message" | "private"
    pub username: String,
    pub room: String,
    pub content: String,
}

// 新增：私聊请求结构
#[derive(Deserialize)]
pub struct PrivateMessageRequest {
    pub target: String,   // 对方用户名
    pub content: String,
}

// 新增：会话列表项
#[derive(Serialize)]
pub struct ConversationItem {
    pub conv_id: String,
    pub conv_type: String,   // "private" | "group"
    pub name: String,        // 私聊显示对方名字，群聊显示房间名
    pub last_content: Option<String>,
    pub last_time: Option<String>,
}

#[derive(Serialize, Clone)]
pub struct ServerMessage {
    #[serde(rename = "type")]
    pub msg_type: String,
    pub username: String,
    pub content: String,
}

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
