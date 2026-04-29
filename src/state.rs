use crate::db::DbPool;
use crate::models::ClientMessage;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tokio::sync::{RwLock, broadcast, mpsc};

/// 群聊频道：group_id → broadcast::Sender（一对多）
pub type GroupRoomMap = Arc<RwLock<HashMap<String, broadcast::Sender<ClientMessage>>>>;

/// 私聊频道：username → mpsc::Sender（一对一）
pub type PrivateRoomMap = Arc<RwLock<HashMap<String, mpsc::Sender<ClientMessage>>>>;

/// 在线用户：group_id → 在线用户名集合（仅群聊）
pub type OnlineMap = Arc<RwLock<HashMap<String, HashSet<String>>>>;

#[derive(Clone)]
pub struct AppState {
    pub group_rooms: GroupRoomMap,
    pub private_rooms: PrivateRoomMap,
    pub online: OnlineMap,
    pub db: DbPool,
}

impl AppState {
    pub fn new(db: DbPool) -> Self {
        Self {
            group_rooms: Arc::new(RwLock::new(HashMap::new())),
            private_rooms: Arc::new(RwLock::new(HashMap::new())),
            online: Arc::new(RwLock::new(HashMap::new())),
            db,
        }
    }

    /// 服务启动时从数据库恢复所有群组的 broadcast channel
    pub async fn restore_group_rooms(&self) {
        let group_ids: Vec<String> =
            sqlx::query_scalar("SELECT group_id FROM groups")
                .fetch_all(&self.db)
                .await
                .unwrap_or_default();

        let mut rooms = self.group_rooms.write().await;
        for id in group_ids {
            let (tx, _) = broadcast::channel(64);
            rooms.insert(id, tx);
        }
    }
}
