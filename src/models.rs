use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize, Clone)]
pub struct ClientMessage {
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

#[derive(Deserialize)]
pub struct LoginRequest {
    pub username: String,
}

#[derive(Serialize)]
pub struct LoginResponse {
    pub token: String,
}