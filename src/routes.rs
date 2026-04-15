use std::collections::HashMap;

use crate::auth;
use crate::db;
use crate::models::ConversationItem;
use crate::models::{LoginRequest, LoginResponse};
use crate::ws::handler_socket;
use crate::{
    auth::{auth_middleware, hash_password, verify_password},
    models::{RegisterRequest, RegisterResponse},
    state::AppState,
};
use axum::response::IntoResponse;
use axum::{
    Json, Router,
    http::StatusCode,
    extract::{Path, Query, State, WebSocketUpgrade},
    middleware::from_fn,
    response::Html,
    routing::{get, post},
};
use tower_http::services::ServeDir;

pub fn app(state: AppState) -> Router {
    // 公开路由（不需要登录）
    let public = Router::new()
        .route("/login", get(login_page))
        .route("/api/login", post(login))
        .route("/register", get(register_page))
        .route("/api/register", post(register))
        .with_state(state.clone());

    // 受保护路由（需要登录，统一经过 auth_middleware）
    let protected = Router::new()
        .route("/", get(index))
        .route("/ws", get(ws_handler))
        .route("/api/rooms/:room/members", get(room_members))
        .route("/api/conversations", get(get_conversations)) // 新增
        .route("/api/private/:target/history", get(private_history)) // 新增
        .route_layer(from_fn(auth_middleware))
        .with_state(state.clone());

    let app = public
        .merge(protected)
        .nest_service("/static", ServeDir::new("static"));
    app
}

pub async fn index() -> Html<&'static str> {
    Html(include_str!("../static/index.html"))
}

// 新增 handler
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
        ).into_response(),
    }
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
) -> Result<Json<LoginResponse>, (axum::http::StatusCode, Json<serde_json::Value>)> {
    let username = payload.username.trim().to_string();
    if username.is_empty() {
        return Err((
            axum::http::StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": "用户名不能为空" })),
        ));
    }
    if payload.password.is_empty() {
        return Err((
            axum::http::StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": "密码不能为空" })),
        ));
    }

    let hash = db::get_password_hash(&state.db, &username)
        .await
        .map_err(|e| {
            (
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": e.to_string() })),
            )
        })?;

    let hash = match hash {
        Some(h) => h,
        None => {
            return Err((
                axum::http::StatusCode::UNAUTHORIZED,
                Json(serde_json::json!({ "error": "用户名或密码错误" })),
            ));
        }
    };

    let password_ok = verify_password(&payload.password, &hash).map_err(|e| {
        (
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e.to_string() })),
        )
    })?;

    if !password_ok {
        return Err((
            axum::http::StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({ "error": "用户名或密码错误" })),
        ));
    }

    let token = auth::sign_token(&username).map_err(|e| {
        (
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e.to_string() })),
        )
    })?;

    Ok(Json(LoginResponse { token }))
}
pub async fn register(
    State(state): State<AppState>,
    Json(payload): Json<RegisterRequest>,
) -> Result<Json<RegisterResponse>, (axum::http::StatusCode, Json<serde_json::Value>)> {
    let username = payload.username.trim().to_string();
    if username.is_empty() {
        return Err((
            axum::http::StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": "用户名不能为空" })),
        ));
    }
    if payload.password.len() < 6 {
        return Err((
            axum::http::StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": "密码至少 6 个字符" })),
        ));
    }

    let password_hash = hash_password(&payload.password).map_err(|e| {
        (
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e.to_string() })),
        )
    })?;

    let created = db::register_user(&state.db, &username, &password_hash)
        .await
        .map_err(|e| {
            (
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": e.to_string() })),
            )
        })?;

    if !created {
        return Err((
            axum::http::StatusCode::CONFLICT,
            Json(serde_json::json!({ "error": "用户名已被占用" })),
        ));
    }

    let token = auth::sign_token(&username).map_err(|e| {
        (
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e.to_string() })),
        )
    })?;

    Ok(Json(RegisterResponse { token }))
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

pub async fn login_page() -> Html<&'static str> {
    Html(include_str!("../static/login.html"))
}

pub async fn register_page() -> Html<&'static str> {
    Html(include_str!("../static/register.html"))
}
