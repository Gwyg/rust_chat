use std::collections::HashMap;

use crate::db;
use crate::state::AppState;
use axum::{
    Json,
    extract::{Query, State},
    http::StatusCode,
    response::IntoResponse,
};
use serde::{Deserialize, Serialize};

#[derive(Serialize)]
pub struct UnreadItem {
    #[serde(rename = "type")]
    pub session_type: String,   // "group" | "private"
    pub id: String,             // group_id 或 对方 username
    pub count: i64,
}

#[derive(Serialize)]
pub struct UnreadResponse {
    pub items: Vec<UnreadItem>,
}

/// GET /api/unread?username=xxx
pub async fn get_unread(
    State(state): State<AppState>,
    Query(params): Query<HashMap<String, String>>,
) -> impl IntoResponse {
    let username = params.get("username").cloned().unwrap_or_default();
    let mut items: Vec<UnreadItem> = Vec::new();

    // ── 群聊未读 ──
    if let Ok(groups) = db::get_user_groups(&state.db, &username).await {
        for g in groups {
            if let Ok(count) = db::get_unread_count(&state.db, &username, "group", &g.group_id).await {
                if count > 0 {
                    items.push(UnreadItem {
                        session_type: "group".into(),
                        id: g.group_id,
                        count,
                    });
                }
            }
        }
    }

    // ── 私聊未读 ──
    if let Ok(convs) = db::get_user_conversations(&state.db, &username).await {
        for conv in convs {
            if conv.conv_type != "private" { continue; }
            if let Ok(count) = db::get_unread_count(&state.db, &username, "private", &conv.conv_id).await {
                if count > 0 {
                    items.push(UnreadItem {
                        session_type: "private".into(),
                        id: conv.name,   // name = 对方 username
                        count,
                    });
                }
            }
        }
    }

    (StatusCode::OK, Json(UnreadResponse { items }))
}

#[derive(Deserialize)]
pub struct MarkReadRequest {
    pub username: String,
    #[serde(rename = "type")]
    pub session_type: String,
    pub id: String,   // group_id 或 对方 username
}

/// POST /api/read
pub async fn mark_read(
    State(state): State<AppState>,
    Json(body): Json<MarkReadRequest>,
) -> impl IntoResponse {
    let session_id = match body.session_type.as_str() {
        "group" => body.id.clone(),
        "private" => {
            // conv_id = 字典序小的在前
            let u = body.username.as_str();
            let t = body.id.as_str();
            if u <= t { format!("{}_{}", u, t) } else { format!("{}_{}", t, u) }
        }
        _ => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({ "error": "无效的 type" })),
            ).into_response();
        }
    };

    match db::mark_session_read(&state.db, &body.username, &body.session_type, &session_id).await {
        Ok(_) => (StatusCode::OK, Json(serde_json::json!({ "ok": true }))).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e.to_string() })),
        ).into_response(),
    }
}