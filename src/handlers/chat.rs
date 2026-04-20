use std::collections::HashMap;

use crate::auth::token::verify_token;
use crate::db;
use crate::models::ConversationItem;
use crate::models::RecallRequest;
use crate::state::AppState;
use axum::{Json, extract::{Path, Query, State}, http::{HeaderMap, StatusCode}, response::{Html, IntoResponse}};
pub async fn index() -> Html<&'static str> {
    Html(include_str!("../../static/index.html"))
}

fn extract_username(headers: &HeaderMap) -> Option<String> {
    if let Some(cookie) = headers.get("cookie") {
        if let Ok(s) = cookie.to_str() {
            for part in s.split(';') {
                let part = part.trim();
                if let Some(val) = part.strip_prefix("chat_token=") {
                    if let Ok(username) = verify_token(val) {
                        return Some(username);
                    }
                }
            }
        }
    }
    if let Some(auth) = headers.get("authorization") {
        if let Ok(s) = auth.to_str() {
            if let Some(token) = s.strip_prefix("Bearer ") {
                if let Ok(username) = verify_token(token) {
                    return Some(username);
                }
            }
        }
    }
    None
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

pub async fn get_conversations(
    State(state): State<AppState>,
    Query(params): Query<HashMap<String, String>>,
) -> Json<Vec<ConversationItem>> {
    let username = params.get("username").cloned().unwrap_or_default();
    match db::get_user_conversations(&state.db, &username).await {
        Ok(list) => Json(list),
        Err(e) => {
            tracing::error!("获取会话列表失败: {}", e);
            Json(vec![])
        }
    }
}

pub async fn private_history(
    State(state): State<AppState>,
    Path(target): Path<String>,
    Query(params): Query<HashMap<String, String>>,
) -> impl IntoResponse {
    let username = params.get("username").cloned().unwrap_or_default();
    let u_ref = username.as_str();
    let t_ref = target.as_str();
    let before_id = params.get("before_id").and_then(|s| s.parse::<i64>().ok());
    let limit: i64 = params.get("limit").and_then(|s| s.parse::<i64>().ok()).unwrap_or(50);

    let conv_id = if u_ref <= t_ref {
        format!("{}_{}", u_ref, t_ref)
    } else {
        format!("{}_{}", t_ref, u_ref)
    };

    match db::get_private_history_paginated(&state.db, &conv_id, before_id, limit).await {
        Ok(result) => (StatusCode::OK, Json(serde_json::json!({
            "messages": result.messages,
            "min_id": result.min_id,
            "has_more": result.has_more,
        }))).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e.to_string() })),
        )
        .into_response(),
    }
}

pub async fn list_users(
    State(state): State<AppState>,
) -> Json<Vec<String>> {
    match db::get_all_users(&state.db).await {
        Ok(users) => Json(users),
        Err(e) => {
            tracing::error!("获取用户列表失败: {}", e);
            Json(vec![])
        }
    }
}

pub async fn room_history_paginated(
    State(state): State<AppState>,
    Path(room): Path<String>,
    Query(params): Query<HashMap<String, String>>,
) -> impl IntoResponse {
    let before_id = params.get("before_id").and_then(|s| s.parse::<i64>().ok());
    let limit: i64 = params.get("limit").and_then(|s| s.parse::<i64>().ok()).unwrap_or(50);

    match db::get_room_history_paginated(&state.db, &room, before_id, limit).await {
        Ok(result) => (StatusCode::OK, Json(serde_json::json!({
            "messages": result.messages,
            "min_id": result.min_id,
            "has_more": result.has_more,
        }))).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e.to_string() })),
        )
        .into_response(),
    }
}

/// POST /api/messages/:message_id/recall
pub async fn recall_message(
    State(state): State<AppState>,
    Path(message_id): Path<i64>,
    headers: HeaderMap,
    Json(body): Json<RecallRequest>,
) -> impl IntoResponse {
    let me = match extract_username(&headers) {
        Some(u) => u,
        None => return (StatusCode::UNAUTHORIZED, Json(serde_json::json!({ "error": "未登录" }))).into_response(),
    };

    let result = if body.msg_type == "group" {
        match db::recall_group_message(&state.db, message_id, &body.room, &me).await {
            Ok(r) => r,
            Err(e) => return (StatusCode::BAD_REQUEST, Json(serde_json::json!({ "error": e.to_string() }))).into_response(),
        }
    } else if body.msg_type == "private" {
        match db::recall_private_message(&state.db, message_id, &body.room, &me).await {
            Ok(r) => r,
            Err(e) => return (StatusCode::BAD_REQUEST, Json(serde_json::json!({ "error": e.to_string() }))).into_response(),
        }
    } else {
        return (StatusCode::BAD_REQUEST, Json(serde_json::json!({ "error": "不支持的消息类型" }))).into_response();
    };

    broadcast_recall(&state, &body.msg_type, &result.room, &me, message_id).await;

    (StatusCode::OK, Json(serde_json::json!({
        "ok": true,
        "message_id": message_id,
    }))).into_response()
}

async fn broadcast_recall(state: &AppState, msg_type: &str, room: &str, sender: &str, message_id: i64) {
    if msg_type == "group" {
        let tx = state.group_rooms.read().await.get(room).cloned();
        if let Some(tx) = tx {
            use crate::models::ClientMessage;
            let _ = tx.send(ClientMessage {
                msg_type: "recall".into(),
                username: sender.into(),
                room: room.into(),
                content: format!("{} 撤回了一条消息", sender),
                message_id: Some(message_id),
                ..Default::default()
            });
        }
        let online_members = {
            state.online.read().await.get(room).cloned().unwrap_or_default()
        };
        if let Ok(all_members) = db::get_group_members(&state.db, room).await {
            for member in &all_members {
                if member.username != sender && !online_members.contains(&member.username) {
                    let _ = db::save_offline_message(
                        &state.db, sender, &member.username,
                        &format!("{} 撤回了一条消息", sender),
                        "recall", room,
                    ).await;
                }
            }
        }
    } else if msg_type == "private" {
        let target = room.split('_').find(|&p| p != sender).unwrap_or("");
        if target.is_empty() { return; }
        let tx = state.private_rooms.read().await.get(target).cloned();
        if let Some(tx) = tx {
            use crate::models::ClientMessage;
            let _ = tx.send(ClientMessage {
                msg_type: "recall".into(),
                username: sender.into(),
                room: room.into(),
                content: format!("{} 撤回了一条消息", sender),
                message_id: Some(message_id),
                ..Default::default()
            }).await;
        } else {
            let _ = db::save_offline_message(
                &state.db, sender, target,
                &format!("{} 撤回了一条消息", sender),
                "recall", room,
            ).await;
        }
    }
}