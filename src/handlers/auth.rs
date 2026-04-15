use crate::auth::password::{hash_password, verify_password};
use crate::auth::token::sign_token;
use crate::db;
use crate::models::{LoginRequest, LoginResponse, RegisterRequest, RegisterResponse};
use crate::state::AppState;
use axum::{Json, extract::State, http::StatusCode, response::Html};

pub async fn login_page() -> Html<&'static str> {
    Html(include_str!("../../static/login.html"))
}

pub async fn register_page() -> Html<&'static str> {
    Html(include_str!("../../static/register.html"))
}

pub async fn login(
    State(state): State<AppState>,
    Json(payload): Json<LoginRequest>,
) -> Result<Json<LoginResponse>, (StatusCode, Json<serde_json::Value>)> {
    let username = payload.username.trim().to_string();
    if username.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": "用户名不能为空" })),
        ));
    }
    if payload.password.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": "密码不能为空" })),
        ));
    }

    let hash = db::get_password_hash(&state.db, &username)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": e.to_string() })),
            )
        })?;

    let hash = match hash {
        Some(h) => h,
        None => {
            return Err((
                StatusCode::UNAUTHORIZED,
                Json(serde_json::json!({ "error": "用户名或密码错误" })),
            ));
        }
    };

    let password_ok = verify_password(&payload.password, &hash).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e.to_string() })),
        )
    })?;

    if !password_ok {
        return Err((
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({ "error": "用户名或密码错误" })),
        ));
    }

    let token = sign_token(&username).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e.to_string() })),
        )
    })?;

    Ok(Json(LoginResponse { token }))
}

pub async fn register(
    State(state): State<AppState>,
    Json(payload): Json<RegisterRequest>,
) -> Result<Json<RegisterResponse>, (StatusCode, Json<serde_json::Value>)> {
    let username = payload.username.trim().to_string();
    if username.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": "用户名不能为空" })),
        ));
    }
    if payload.password.len() < 6 {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": "密码至少 6 个字符" })),
        ));
    }

    let password_hash = hash_password(&payload.password).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e.to_string() })),
        )
    })?;

    let created = db::register_user(&state.db, &username, &password_hash)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": e.to_string() })),
            )
        })?;

    if !created {
        return Err((
            StatusCode::CONFLICT,
            Json(serde_json::json!({ "error": "用户名已被占用" })),
        ));
    }

    let token = sign_token(&username).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e.to_string() })),
        )
    })?;

    Ok(Json(RegisterResponse { token }))
}