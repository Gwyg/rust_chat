mod db;
mod models;
mod routes;
mod state;
mod ws;
mod auth;

use state::AppState;
use tracing::info;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    // 初始化数据库
    let pool = db::create_pool("chat.db").await.expect("数据库初始化失败");

    // 运行迁移
    sqlx::migrate!("./data")
        .run(&pool)
        .await
        .expect("数据库迁移失败");

    let state = AppState::new(pool);
    state.init_rooms(vec!["Java群", "Rust群", "闲聊"]).await;

    let app = routes::app(state);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000")
        .await
        .expect("bind failed");
    info!("服务启动: http://localhost:3000");

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


