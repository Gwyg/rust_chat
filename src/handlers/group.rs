use crate::auth::token::verify_token;
use crate::db;
use crate::models::{
    CreateGroupRequest, GroupMemberRequest,
    UpdateGroupAvatarRequest, UpdateGroupNoticeRequest,
};
use crate::state::AppState;
use axum::{
    Json,
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
};

/// 从请求头提取 token 并验证，返回用户名（复用 friends.rs 的逻辑）
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

/// POST /api/groups — 创建群组
pub async fn create_group(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<CreateGroupRequest>,
) -> impl IntoResponse {
    let me = match extract_username(&headers) {
        Some(u) => u,
        None => return (StatusCode::UNAUTHORIZED, Json(serde_json::json!({ "error": "未登录" }))).into_response(),
    };

    if body.name.trim().is_empty() {
        return (StatusCode::BAD_REQUEST, Json(serde_json::json!({ "error": "群名称不能为空" }))).into_response();
    }

    match db::create_group(&state.db, &me, body.name.trim()).await {
        Ok(group_id) => {
            // 同步在 AppState.rooms 中注册该群的广播频道
            let mut rooms = state.rooms.write().await;
            let (tx, _) = tokio::sync::broadcast::channel(64);
            rooms.insert(group_id.clone(), tx);
            drop(rooms);

            (StatusCode::OK, Json(serde_json::json!({ "group_id": group_id }))).into_response()
        }
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "error": e.to_string() }))).into_response(),
    }
}

/// GET /api/groups — 获取当前用户所在的所有群组
pub async fn list_groups(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> impl IntoResponse {
    let me = match extract_username(&headers) {
        Some(u) => u,
        None => return (StatusCode::UNAUTHORIZED, Json(serde_json::json!({ "error": "未登录" }))).into_response(),
    };

    match db::get_user_groups(&state.db, &me).await {
        Ok(list) => Json(list).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "error": e.to_string() }))).into_response(),
    }
}

/// GET /api/groups/:group_id/members — 获取群成员列表
pub async fn list_group_members(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(group_id): Path<String>,
) -> impl IntoResponse {
    let _me = match extract_username(&headers) {
        Some(u) => u,
        None => return (StatusCode::UNAUTHORIZED, Json(serde_json::json!({ "error": "未登录" }))).into_response(),
    };

    match db::get_group_members(&state.db, &group_id).await {
        Ok(list) => Json(list).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "error": e.to_string() }))).into_response(),
    }
}

/// POST /api/groups/members/add — 邀请用户加入群组
pub async fn add_member(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<GroupMemberRequest>,
) -> impl IntoResponse {
    let me = match extract_username(&headers) {
        Some(u) => u,
        None => return (StatusCode::UNAUTHORIZED, Json(serde_json::json!({ "error": "未登录" }))).into_response(),
    };

    // 校验操作者是群主或管理员
    match db::get_group_detail(&state.db, &body.group_id, &me).await {
        Ok(Some(g)) if g.role == "owner" || g.role == "admin" => {}
        Ok(Some(_)) => return (StatusCode::FORBIDDEN, Json(serde_json::json!({ "error": "权限不足" }))).into_response(),
        Ok(None) => return (StatusCode::NOT_FOUND, Json(serde_json::json!({ "error": "群组不存在或你不在此群" }))).into_response(),
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "error": e.to_string() }))).into_response(),
    }

    match db::add_group_member(&state.db, &body.group_id, &body.username).await {
        Ok(true)  => (StatusCode::OK, Json(serde_json::json!({ "ok": true }))).into_response(),
        Ok(false) => (StatusCode::CONFLICT, Json(serde_json::json!({ "error": "该用户已在群组中" }))).into_response(),
        Err(e)    => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "error": e.to_string() }))).into_response(),
    }
}

/// POST /api/groups/members/remove — 踢出群成员
pub async fn remove_member(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<GroupMemberRequest>,
) -> impl IntoResponse {
    let me = match extract_username(&headers) {
        Some(u) => u,
        None => return (StatusCode::UNAUTHORIZED, Json(serde_json::json!({ "error": "未登录" }))).into_response(),
    };

    match db::remove_group_member(&state.db, &body.group_id, &me, &body.username).await {
        Ok(_)  => (StatusCode::OK, Json(serde_json::json!({ "ok": true }))).into_response(),
        Err(e) => (StatusCode::FORBIDDEN, Json(serde_json::json!({ "error": e.to_string()}))).into_response(),
    }
}

/// PUT /api/groups/notice — 更新群公告
pub async fn update_notice(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<UpdateGroupNoticeRequest>,
) -> impl IntoResponse {
    let me = match extract_username(&headers) {
        Some(u) => u,
        None => return (StatusCode::UNAUTHORIZED, Json(serde_json::json!({ "error": "未登录" }))).into_response(),
    };

    match db::update_group_notice(&state.db, &body.group_id, &me, &body.notice).await {
        Ok(_)  => (StatusCode::OK, Json(serde_json::json!({ "ok": true }))).into_response(),
        Err(e) => (StatusCode::FORBIDDEN, Json(serde_json::json!({ "error": e.to_string() }))).into_response(),
    }
}

/// PUT /api/groups/avatar — 更新群头像
pub async fn update_avatar(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<UpdateGroupAvatarRequest>,
) -> impl IntoResponse {
    let me = match extract_username(&headers) {
        Some(u) => u,
        None => return (StatusCode::UNAUTHORIZED, Json(serde_json::json!({ "error": "未登录" }))).into_response(),
    };

    match db::update_group_avatar(&state.db, &body.group_id, &me, &body.avatar).await {
        Ok(_)  => (StatusCode::OK, Json(serde_json::json!({ "ok": true }))).into_response(),
        Err(e) => (StatusCode::FORBIDDEN, Json(serde_json::json!({ "error": e.to_string()}))).into_response(),
    }
}

/// DELETE /api/groups/:group_id — 解散群组
pub async fn dissolve_group(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(group_id): Path<String>,
) -> impl IntoResponse {
    let me = match extract_username(&headers) {
        Some(u) => u,
        None => return (StatusCode::UNAUTHORIZED, Json(serde_json::json!({ "error": "未登录" }))).into_response(),
    };

    match db::dissolve_group(&state.db, &group_id, &me).await {
        Ok(_) => {
            // 从 AppState.rooms 中移除该群的广播频道
            state.rooms.write().await.remove(&group_id);
            (StatusCode::OK, Json(serde_json::json!({ "ok": true }))).into_response()
        }
        Err(e) => (StatusCode::FORBIDDEN, Json(serde_json::json!({ "error": e.to_string() }))).into_response(),
    }
}