use serde::{Deserialize, Serialize};

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
    pub status: String,
}

#[derive(Deserialize)]
pub struct FriendRequest {
    pub target: String,
}