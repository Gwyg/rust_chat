mod message;

use crate::auth::token::verify_token;
use crate::db;
use crate::models::{ClientMessage, ServerMessage};
use crate::state::AppState;
use crate::ws::message::{
    find_room, forward_to_client, handle_client_message, parse_join_message,
};
use axum::extract::ws::{Message, WebSocket};
use std::time::Duration;
use tokio::sync::broadcast;
use tokio::time::interval;
use tracing::{info, warn};

pub async fn handler_socket(mut socket: WebSocket, state: AppState, token: String) {
    let username = match verify_token(&token) {
        Ok(name) => name,
        Err(_) => {
            message::send_error(&mut socket, "无效的 token, 请重新登录").await;
            return;
        }
    };

    let join_msg = match parse_join_message(&mut socket).await {
        Some(msg) => msg,
        None => return,
    };
    let room = join_msg.room;

    let tx = match find_room(&state, &room, &mut socket).await {
        Some(tx) => tx,
        None => return,
    };

    info!("用户 {} 加入房间 {}", username, room);
    state.online.write().await.entry(room.clone()).or_default().insert(username.clone());

    // 推送历史
    if let Ok(messages) = db::get_room_history(&state.db, &room, 50).await {
        for msg in messages {
            let server_msg = ServerMessage {
                msg_type: "history".into(),
                username: msg.username,
                content: msg.content,
            };
            if socket
                .send(Message::Text(serde_json::to_string(&server_msg).unwrap()))
                .await
                .is_err()
            {
                return;
            }
        }
    }

    broadcast_system(&tx, &room, &format!("用户 {} 加入房间", username)).await;

    let mut rx = tx.subscribe();
    let mut heartbeat = interval(Duration::from_secs(30));
    heartbeat.tick().await;

    loop {
        tokio::select! {
            _ = heartbeat.tick() => {
                if socket.send(Message::Ping(vec![].into())).await.is_err() {
                    info!("用户 {} 心跳超时", username);
                    broadcast_system(&tx, &room, &format!("{} 离开了房间", username)).await;
                    break;
                }
            }
            msg = socket.recv() => {
                match msg {
                    Some(Ok(Message::Text(text))) => {
                        handle_client_message(&state, &text, &username, &room).await;
                    }
                    Some(Ok(Message::Pong(_))) => {}
                    _ => {
                        info!("用户 {} 断开", username);
                        break;
                    }
                }
            }
            msg = rx.recv() => match msg {
                Ok(client_msg) => {
                    if !forward_to_client(&mut socket, client_msg).await {
                        break;
                    }
                }
                Err(broadcast::error::RecvError::Lagged(n)) => {
                    warn!("用户 {} 丢失 {} 条消息", username, n);
                    continue;
                }
                Err(broadcast::error::RecvError::Closed) => break,
            },
        }
    }

    {
        let mut online = state.online.write().await;
        if let Some(members) = online.get_mut(&room) {
            members.remove(&username);
        }
    }
    broadcast_system(&tx, &room, &format!("{} 离开了房间", username)).await;
    info!("用户 {} 离开房间 {}", username, room);
}

async fn broadcast_system(tx: &broadcast::Sender<ClientMessage>, room: &str, content: &str) {
    let _ = tx.send(ClientMessage {
        username: "系统".into(),
        room: room.into(),
        content: content.into(),
        msg_type: "message".into(),
    });
}