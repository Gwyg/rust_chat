use serde::{Deserialize, Serialize};

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