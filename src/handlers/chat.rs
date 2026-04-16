use std::collections::HashMap;

use crate::db;
use crate::models::ConversationItem;
use crate::state::AppState;
use axum::{Json, extract::{Path, Query, State}, http::StatusCode, response::{Html, IntoResponse}};

pub async fn index() -> Html<&'static str> {
    Html(include_str!("../../static/index.html"))
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

    let conv_id = if u_ref <= t_ref {
        format!("{}_{}", u_ref, t_ref)
    } else {
        format!("{}_{}", t_ref, u_ref)
    };

    match db::get_private_history(&state.db, &conv_id, 50).await {
        Ok(msgs) => (StatusCode::OK, Json(msgs)).into_response(),
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