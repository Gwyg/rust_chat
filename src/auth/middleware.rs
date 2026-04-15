use crate::auth::token::verify_token;
use axum::{
    extract::Request,
    http::StatusCode,
    middleware::Next,
    response::{IntoResponse, Response},
};

fn extract_token(request: &Request) -> Option<String> {
    if let Some(cookie) = request.headers().get("cookie") {
        if let Ok(s) = cookie.to_str() {
            for part in s.split(';') {
                let part = part.trim();
                if let Some(val) = part.strip_prefix("chat_token=") {
                    return Some(val.to_string());
                }
            }
        }
    }
    if let Some(auth) = request.headers().get("authorization") {
        if let Ok(s) = auth.to_str() {
            if let Some(token) = s.strip_prefix("Bearer ") {
                return Some(token.to_string());
            }
        }
    }
    None
}

pub async fn auth_middleware(request: Request, next: Next) -> Response {
    let token = extract_token(&request);
    let is_browser = request
        .headers()
        .get("accept")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.contains("text/html"))
        .unwrap_or(false);

    match token {
        Some(t) => match verify_token(&t) {
            Ok(_) => next.run(request).await,
            Err(_) => {
                if is_browser {
                    (StatusCode::FOUND, [("Location", "/login")]).into_response()
                } else {
                    (StatusCode::UNAUTHORIZED, axum::Json(serde_json::json!({
                        "error": "token 已过期，请重新登录"
                    }))).into_response()
                }
            }
        },
        None => {
            if is_browser {
                (StatusCode::FOUND, [("Location", "/login")]).into_response()
            } else {
                (StatusCode::UNAUTHORIZED, axum::Json(serde_json::json!({
                    "error": "未登录"
                }))).into_response()
            }
        }
    }
}