mod auth;
mod chat;
mod ws;

pub use auth::*;
pub use chat::*;
pub use ws::*;

use crate::auth::middleware::auth_middleware;
use crate::state::AppState;
use axum::{Router, middleware::from_fn, routing::{get, post}};
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
        .route("/api/conversations", get(get_conversations))
        .route("/api/private/:target/history", get(private_history))
        .route_layer(from_fn(auth_middleware))
        .with_state(state.clone());

    public.merge(protected)
        .nest_service("/static", ServeDir::new("static"))
}