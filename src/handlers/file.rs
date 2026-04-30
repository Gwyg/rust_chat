use crate::auth::token::verify_token;
use crate::db;
use crate::state::AppState;
use axum::{
    body::Body,
    extract::{Multipart, Path, State},
    http::{HeaderMap, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
};
use std::path::Path as StdPath;
use uuid::Uuid;

/// 从请求头提取 username（与 friends/group 逻辑一致）
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

/// POST /api/upload — 上传文件（multipart/form-data，字段名 file）
pub async fn upload_file(
    State(state): State<AppState>,
    headers: HeaderMap,
    mut multipart: Multipart,
) -> impl IntoResponse {
    let me = match extract_username(&headers) {
        Some(u) => u,
        None => return err_resp(StatusCode::UNAUTHORIZED, "未登录"),
    };

    let file_field = match multipart.next_field().await {
        Ok(Some(f)) => f,
        _ => return err_resp(StatusCode::BAD_REQUEST, "未找到文件"),
    };

    let original_name = file_field.file_name().unwrap_or("unknown").to_string();
    let content_type = file_field
        .content_type()
        .map(|s| s.to_string())
        .unwrap_or_else(|| {
            mime_guess::from_path(&original_name)
                .first()
                .map(|m| m.to_string())
                .unwrap_or_else(|| "application/octet-stream".into())
        });

    let bytes = match file_field.bytes().await {
        Ok(b) => b,
        Err(_) => return err_resp(StatusCode::BAD_REQUEST, "读取文件失败"),
    };

    // 100MB 限制
    if bytes.len() > 100 * 1024 * 1024 {
        return err_resp(StatusCode::PAYLOAD_TOO_LARGE, "文件大小超过 100MB 限制");
    }

    // 生成存储路径
    let ext = StdPath::new(&original_name)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("");
    let file_id = Uuid::new_v4().to_string();
    let safe_name = if ext.is_empty() {
        format!("{}.bin", file_id)
    } else {
        format!("{}.{}", file_id, ext)
    };

    

    let upload_dir = match db::ensure_upload_dir() {
        Ok(d) => d,
        Err(e) => {
            return err_resp(StatusCode::INTERNAL_SERVER_ERROR, &format!("创建目录失败: {}", e))
        }
    };
    let storage_path = format!("{}/{}", upload_dir, safe_name);

    // 写磁盘
    if let Err(e) = std::fs::write(&storage_path, &bytes) {
        return err_resp(StatusCode::INTERNAL_SERVER_ERROR, &format!("保存文件失败: {}", e));
    }

    // 写数据库
    if let Err(e) = db::save_file_record(
        &state.db, &file_id, &original_name, &content_type,
        bytes.len() as i64, &storage_path, &me,
    )
    .await
    {
        let _ = std::fs::remove_file(&storage_path);
        return err_resp(
            StatusCode::INTERNAL_SERVER_ERROR,
            &format!("保存记录失败: {}", e),
        );
    }

    (
        StatusCode::OK,
        axum::Json(serde_json::json!({
            "file_id": file_id,
            "filename": original_name,
            "mime_type": content_type,
            "size": bytes.len(),
        })),
    )
        .into_response()
}

/// GET /api/download/{file_id} — 下载文件（需登录鉴权）
pub async fn download_file(
    State(state): State<AppState>,
    Path(file_id): Path<String>,
    headers: HeaderMap,
) -> impl IntoResponse {
    match extract_username(&headers) {
        Some(_) => {}
        None => return err_resp(StatusCode::UNAUTHORIZED, "未登录"),
    }

    let record = match db::get_file_record(&state.db, &file_id).await {
        Ok(Some(r)) => r,
        Ok(None) => return err_resp(StatusCode::NOT_FOUND, "文件不存在"),
        Err(e) => {
            return err_resp(StatusCode::INTERNAL_SERVER_ERROR, &format!("查询失败: {}", e))
        }
    };

    let file_bytes = match std::fs::read(&record.storage_path) {
        Ok(b) => b,
        Err(_) => return err_resp(StatusCode::NOT_FOUND, "文件已被删除"),
    };

    let mut resp = Response::new(Body::from(file_bytes));
    resp.headers_mut().insert(
        "Content-Type",
        HeaderValue::from_str(&record.mime_type)
            .unwrap_or_else(|_| HeaderValue::from_static("application/octet-stream")),
    );
    let encoded_name = url_encode_filename(&record.filename);
    resp.headers_mut().insert(
        "Content-Disposition",
        HeaderValue::from_str(&format!("attachment; filename=\"{}\"", encoded_name))
            .unwrap_or_else(|_| HeaderValue::from_static("attachment")),
    );
    resp.headers_mut().insert(
        "Content-Length",
        HeaderValue::from_str(record.file_size.to_string().as_str())
            .unwrap_or_else(|_| HeaderValue::from_static("0")),
    );

    resp
}

fn err_resp(status: StatusCode, msg: &str) -> Response {
    (status, axum::Json(serde_json::json!({ "error": msg }))).into_response()
}

/// 文件名 URL-encode（支持中文）
fn url_encode_filename(name: &str) -> String {
    let mut encoded = String::new();
    for c in name.chars() {
        if c.is_ascii_alphanumeric() || c == '.' || c == '_' || c == '-' || c == ' ' {
            encoded.push(c);
        } else {
            for byte in c.to_string().as_bytes() {
                encoded.push_str(&format!("%{:02X}", byte));
            }
        }
    }
    encoded
}