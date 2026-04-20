use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize, Clone, Default)]
pub struct ClientMessage {
    pub msg_type: String,
    pub username: String,
    pub room: String,
    pub content: String,
    // 文件传输扩展字段
    #[serde(default)]
    pub file_id: Option<String>,
    #[serde(default)]
    pub filename: Option<String>,
    #[serde(default)]
    pub mime_type: Option<String>,
    #[serde(default)]
    pub recalled: bool,
    #[serde(default)]
    pub message_id: Option<i64>,
}

#[derive(Serialize, Clone, Default)]
pub struct ServerMessage {
    #[serde(rename = "type")]
    pub msg_type: String,
    pub username: String,
    pub content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub filename: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mime_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message_id: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub recalled: Option<bool>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct UnreadItem {
    #[serde(rename = "type")]
    pub item_type: String,   // "group" 或 "private"
    pub id: String,          // group_id 或对方 username
    pub count: i64,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct UnreadSummary {
    pub msg_type: String,    // 固定为 "unread_summary"
    pub items: Vec<UnreadItem>,
}