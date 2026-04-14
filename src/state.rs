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

    pub async fn init_rooms(&self, room_names: Vec<&str>) {
        let mut rooms = self.rooms.write().await;
        for name in room_names {
            let (tx, _) = broadcast::channel(64);
            rooms.insert(name.to_string(), tx);
        }
    }
}
