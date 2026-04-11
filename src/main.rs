mod models;
mod state;
mod routes;
mod ws;

use axum::{Router, routing::get};
use tower_http::services::ServeDir;
use tracing::info;
use state::AppState;
use routes::{index, ws_handler};

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let state = AppState::new();
    state.init_rooms(vec!["Java群", "Rust群", "闲聊"]).await;

    let app = Router::new()
        .route("/", get(index))
        .route("/ws", get(ws_handler))
        .nest_service("/static", ServeDir::new("static"))
        .with_state(state);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    info!("服务启动：http://localhost:3000");
    axum::serve(listener, app).await.unwrap();
}