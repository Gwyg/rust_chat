use crate::db::DbPool;

#[derive(sqlx::FromRow)]
pub struct FileRecord {
    pub filename: String,
    pub mime_type: String,
    pub file_size: i64,
    pub storage_path: String,
}

/// 保存文件元数据到数据库
pub async fn save_file_record(
    pool: &DbPool,
    id: &str,
    filename: &str,
    mime_type: &str,
    file_size: i64,
    storage_path: &str,
    uploader: &str,
) -> anyhow::Result<()> {
    sqlx::query(
        "INSERT INTO files (id, filename, mime_type, file_size, storage_path, uploader)
         VALUES (?, ?, ?, ?, ?, ?)"
    )
    .bind(id)
    .bind(filename)
    .bind(mime_type)
    .bind(file_size)
    .bind(storage_path)
    .bind(uploader)
    .execute(pool)
    .await?;
    Ok(())
}

/// 根据 file_id 查询文件元数据
pub async fn get_file_record(pool: &DbPool, id: &str) -> anyhow::Result<Option<FileRecord>> {
    let row = sqlx::query_as::<_, FileRecord>(
        "SELECT id, filename, mime_type, file_size, storage_path, uploader, created_at
         FROM files WHERE id = ?"
    )
    .bind(id)
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

/// 确保上传目录存在，返回目录路径
pub fn ensure_upload_dir() -> anyhow::Result<String> {
    let dir = format!("data/uploads/{}", chrono::Utc::now().format("%Y-%m-%d"));
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}