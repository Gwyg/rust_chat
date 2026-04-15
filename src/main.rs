mod auth;
mod db;
mod handlers;
mod models;
mod state;
mod ws;

use state::AppState;
use tracing::info;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let pool = db::create_pool("chat.db").await.expect("数据库初始化失败");

    sqlx::migrate!("./data")
        .run(&pool)
        .await
        .expect("数据库迁移失败");

    let state = AppState::new(pool);
    state.init_rooms(vec!["Java群", "Rust群", "闲聊"]).await;

    let app = handlers::app(state);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000")
        .await
        .expect("bind failed");
    info!("服务启动: http://127.0.0.1:3000");

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
