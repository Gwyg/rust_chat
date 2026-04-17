use crate::db::DbPool;
use crate::models::ClientMessage;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tokio::sync::{RwLock, broadcast};

pub type RoomMap = Arc<RwLock<HashMap<String, broadcast::Sender<ClientMessage>>>>;
pub type OnlineMap = Arc<RwLock<HashMap<String, HashSet<String>>>>;

#[derive(Clone)]
pub struct AppState {
    pub rooms: RoomMap,
    pub db: DbPool,
    pub online: OnlineMap,
}

impl AppState {
    pub fn new(db: DbPool) -> Self {
        Self {
            rooms: Arc::new(RwLock::new(HashMap::new())),
            db,
            online: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// 服务启动时从数据库恢复所有群组的 broadcast channel
    /// 防止重启后用户进群报「房间不存在」
    pub async fn restore_group_rooms(&self) {
        let group_ids: Vec<String> =
            sqlx::query_scalar("SELECT group_id FROM groups")
                .fetch_all(&self.db)
                .await
                .unwrap_or_default();

        let mut rooms = self.rooms.write().await;
        for id in group_ids {
            let (tx, _) = broadcast::channel(64);
            rooms.insert(id, tx);
        }
    }
}
