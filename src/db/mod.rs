mod message;
mod user;
mod private;
mod friend;
mod group;          // ← 新增这一行
mod offline;
mod file;
mod cursor;


use sqlx::sqlite::SqlitePool;

pub use cursor::*;
pub use file::*;
pub use offline::*;
pub use group::*;
pub use message::*;
pub use user::*;
pub use private::*;
pub use friend::*;
pub use message::PaginatedMessages;

pub type DbPool = SqlitePool;

pub async fn create_pool(db_path: &str) -> anyhow::Result<DbPool> {
    let url = format!("sqlite:{}?mode=rwc", db_path);
    let pool = SqlitePool::connect(&url).await?;
    Ok(pool)
}