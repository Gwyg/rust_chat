mod models;
mod routes;
mod state;
mod ws;

use axum::{Router, routing::get};
use routes::{index, ws_handler};
use state::AppState;
use tower_http::services::ServeDir;
use tracing::info;

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
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .expect("serve failed");
}

async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("failed to install ctrl_c handler");
    };
    ctrl_c.await;
    info!("收到关闭信号，正在停止服务");
}
