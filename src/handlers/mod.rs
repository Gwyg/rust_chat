mod auth;
mod chat;
mod file;
mod friends;
mod group;
mod ws;

pub use auth::*;
pub use chat::*;
pub use file::*;
pub use friends::*;
pub use group::*;
pub use ws::*;

use crate::auth::middleware::auth_middleware;
use crate::state::AppState;
use axum::{
    Router,
    middleware::from_fn,
    routing::{delete, get, post, put},
};
use tower_http::services::ServeDir;

pub fn app(state: AppState) -> Router {
    let public = Router::new()
        .route("/login", get(login_page))
        .route("/api/login", post(login))
        .route("/register", get(register_page))
        .route("/api/register", post(register))
        .with_state(state.clone());

    let protected = Router::new()
        .route("/", get(index))
        .route("/ws", get(ws_handler))
        .route("/api/rooms/:room/members", get(room_members))
        .route("/api/users", get(list_users))
        .route("/api/conversations", get(get_conversations))
        .route("/api/private/:target/history", get(private_history))
        // 好友相关
        .route("/api/friends", get(list_friends))
        .route("/api/friends/request", post(add_friend))
        .route("/api/friends/accept", post(accept_friend))
        .route("/api/friends/:target", delete(delete_friend))
        // 群组相关（新增）
        .route(
            "/api/rooms/{room}/history",
            get(chat::room_history_paginated),
        )
        .route("/api/groups", get(list_groups).post(create_group))
        .route("/api/groups/:group_id", delete(dissolve_group))
        .route("/api/groups/:group_id/members", get(list_group_members))
        .route("/api/groups/members/add", post(add_member))
        .route("/api/groups/members/remove", post(remove_member))
        .route("/api/groups/notice", put(update_notice))
        .route("/api/groups/avatar", put(update_avatar))
        .route("/api/upload", post(upload_file))
        .route("/api/download/{file_id}", get(download_file))
        .route("/api/messages/:message_id/recall", post(recall_message))
        .route_layer(from_fn(auth_middleware))
        .with_state(state.clone());

    public
        .merge(protected)
        .nest_service("/static", ServeDir::new("static"))
}
