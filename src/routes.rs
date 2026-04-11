use std::collections::HashMap;

use axum::{
    Json, extract::{Path, Query, State, WebSocketUpgrade}, response::Html
};
use crate::state::AppState;
use crate::ws::handler_socket;
use crate::models::{LoginRequest, LoginResponse};
use crate::auth;
use crate::db;

pub async fn index() -> Html<&'static str> {
    Html(include_str!("../static/index.html"))
}

pub async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
    Query(params): Query<HashMap<String, String>>,
) -> impl axum::response::IntoResponse {
    let token = params.get("token").cloned().unwrap_or_default();
    ws.on_upgrade(move |socket| handler_socket(socket, state, token))
}

pub async fn login(
    State(state): State<AppState>,
    Json(payload): Json<LoginRequest>,
) -> Result<Json<LoginResponse>, String> {
    let username = payload.username.trim().to_string();
    if username.is_empty() {
        return Err("用户名不能为空".into());
    }

    // 确保用户存在于数据库
    db::ensure_user(&state.db, &username)
        .await
        .map_err(|e| e.to_string())?;

    let token = auth::sign_token(&username).map_err(|e| e.to_string())?;
    Ok(Json(LoginResponse { token }))
}

pub async fn room_members(
    State(state): State<AppState>,
    Path(room): Path<String>,
) -> Json<Vec<String>> {
    let online = state.online.read().await;
    let members = online
        .get(&room)
        .map(|s| {
            let mut v: Vec<String> = s.iter().cloned().collect();
            v.sort();
            v
        })
        .unwrap_or_default();
    Json(members)
}