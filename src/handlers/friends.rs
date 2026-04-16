use crate::auth::token::verify_token;
use crate::db;
use crate::models::FriendRequest;
use crate::state::AppState;
use axum::{
    Json,
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
};

/// 从请求头提取并验证 token，返回 username
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

/// GET /api/friends
pub async fn list_friends(State(state): State<AppState>, headers: HeaderMap) -> impl IntoResponse {
    let me = match extract_username(&headers) {
        Some(u) => u,
        None => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(serde_json::json!({ "error": "未登录" })),
            )
                .into_response();
        }
    };
    match db::get_friends(&state.db, &me).await {
        Ok(list) => Json(list).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}

/// POST /api/friends/request  body: { "target": "bob" }
pub async fn add_friend(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<FriendRequest>,
) -> impl IntoResponse {
    let me = match extract_username(&headers) {
        Some(u) => u,
        None => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(serde_json::json!({ "error": "未登录" })),
            )
                .into_response();
        }
    };
    if me == body.target {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": "不能加自己为好友" })),
        )
            .into_response();
    }
    match db::send_friend_request(&state.db, &me, &body.target).await {
        Ok(_) => (StatusCode::OK, Json(serde_json::json!({ "ok": true }))).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}

/// POST /api/friends/accept  body: { "target": "alice" }
pub async fn accept_friend(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<FriendRequest>,
) -> impl IntoResponse {
    let me = match extract_username(&headers) {
        Some(u) => u,
        None => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(serde_json::json!({ "error": "未登录" })),
            )
                .into_response();
        }
    };
    match db::accept_friend(&state.db, &body.target, &me).await {
        Ok(_) => (StatusCode::OK, Json(serde_json::json!({ "ok": true }))).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}

/// DELETE /api/friends/:target
pub async fn delete_friend(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(target): Path<String>,
) -> impl IntoResponse {
    let me = match extract_username(&headers) {
        Some(u) => u,
        None => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(serde_json::json!({ "error": "未登录" })),
            )
                .into_response();
        }
    };
    match db::delete_friend(&state.db, &me, &target).await {
        Ok(_) => (StatusCode::OK, Json(serde_json::json!({ "ok": true }))).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}
