use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{broadcast, RwLock};
use crate::db::DbPool;
use crate::models::ClientMessage;

pub type RoomMap = Arc<RwLock<HashMap<String, broadcast::Sender<ClientMessage>>>>;

#[derive(Clone)]
pub struct AppState {
    pub rooms: RoomMap,
    pub db: DbPool,
}

impl AppState {
    pub fn new(db: DbPool) -> Self {
        Self {
            rooms: Arc::new(RwLock::new(HashMap::new())),
            db,
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