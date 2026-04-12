use jsonwebtoken::{decode, encode, Algorithm, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use axum::{
    extract::{Request, State},
    http::StatusCode,
    middleware::Next,
    response::{IntoResponse, Response},
};

const SECRET: &[u8] = b"chat_secret_key"; // 后续可改成配置项

#[derive(Serialize, Deserialize)]
pub struct Claims {
    pub username: String,
    pub exp: usize,
}

/// 签发 token
pub fn sign_token(username: &str) -> anyhow::Result<String> {
    let claims = Claims {
        username: username.to_string(),
        exp: 9999999999, // 暂时不过期
    };
    let token = encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(SECRET),
    )?;
    Ok(token)
}

/// 验证 token，返回 username
pub fn verify_token(token: &str) -> anyhow::Result<String> {
    let data = decode::<Claims>(
        token,
        &DecodingKey::from_secret(SECRET),
        &Validation::new(Algorithm::HS256),
    )?;
    Ok(data.claims.username)
}

/// 从请求中提取 token（先找 Cookie，再找 Authorization header）
fn extract_token(request: &Request) -> Option<String> {
    // 从 Cookie 找
    if let Some(cookie_header) = request.headers().get("cookie") {
        if let Ok(s) = cookie_header.to_str() {
            for part in s.split(';') {
                let part = part.trim();
                if let Some(val) = part.strip_prefix("chat_token=") {
                    return Some(val.to_string());
                }
            }
        }
    }
    // 从 Authorization: Bearer <token> 找
    if let Some(auth_header) = request.headers().get("authorization") {
        if let Ok(s) = auth_header.to_str() {
            if let Some(token) = s.strip_prefix("Bearer ") {
                return Some(token.to_string());
            }
        }
    }
    None
}

/// 认证中间件：保护需要登录的路由
/// - 浏览器页面请求（Accept: text/html）→ 302 跳转 /login
/// - API 请求 → 401 JSON
pub async fn auth_middleware(
    request: Request,
    next: Next,
) -> Response {
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
                    (
                        StatusCode::FOUND,
                        [("Location", "/login")],
                    ).into_response()
                } else {
                    (
                        StatusCode::UNAUTHORIZED,
                        axum::Json(serde_json::json!({ "error": "token 已过期，请重新登录" })),
                    ).into_response()
                }
            }
        },
        None => {
            if is_browser {
                (
                    StatusCode::FOUND,
                    [("Location", "/login")],
                ).into_response()
            } else {
                (
                    StatusCode::UNAUTHORIZED,
                    axum::Json(serde_json::json!({ "error": "未登录" })),
                ).into_response()
            }
        }
    }
}