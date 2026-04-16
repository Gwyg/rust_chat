mod message;
mod user;
mod private;
mod friend;

use sqlx::sqlite::SqlitePool;

pub use message::*;
pub use user::*;
pub use private::*;
pub use friend::*;

pub type DbPool = SqlitePool;

pub async fn create_pool(db_path: &str) -> anyhow::Result<DbPool> {
    let url = format!("sqlite:{}?mode=rwc", db_path);
    let pool = SqlitePool::connect(&url).await?;
    Ok(pool)
}