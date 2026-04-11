use axum::{
    extract::{State, WebSocketUpgrade},
    response::Html,
};
use crate::state::AppState;
use crate::ws::handler_socket;

pub async fn index() -> Html<&'static str> {
    Html(include_str!("../static/index.html"))
}

pub async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
) -> impl axum::response::IntoResponse {
    ws.on_upgrade(|socket| handler_socket(socket, state))
}