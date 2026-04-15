use crate::state::AppState;
use crate::ws::handler_socket;
use axum::{
    extract::{Query, State, WebSocketUpgrade},
    response::IntoResponse,
};
use std::collections::HashMap;

pub async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
    Query(params): Query<HashMap<String, String>>,
) -> impl IntoResponse {
    let token = params.get("token").cloned().unwrap_or_default();
    ws.on_upgrade(move |socket| handler_socket(socket, state, token))
}