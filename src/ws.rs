use crate::models::{ClientMessage, ServerMessage};
use crate::state::AppState;
use crate::{auth, db};
use axum::extract::ws::{Message, WebSocket};
use std::time::Duration;
use tokio::sync::broadcast;
use tokio::time::interval;
use tracing::info;

/// WebSocket 核心处理
pub async fn handler_socket(mut socket: WebSocket, state: AppState, token: String) {
    // 验证 token
    let _ = match auth::verify_token(&token) {
        Ok(name) => name,
        Err(_) => {
            send_error(&mut socket, "无效的 token，请重新登录").await;
            return;
        }
    };
    // === 前置校验 ===
    let join_msg = match parse_join_message(&mut socket).await {
        Some(msg) => msg,
        None => return,
    };

    let username = join_msg.username;
    let room = join_msg.room;

    let tx = match find_room(&state, &room, &mut socket).await {
        Some(tx) => tx,
        None => return,
    };

    // === 加入房间 ===
    info!("用户 {} 加入房间 {}", username, room);
    let mut online = state.online.write().await;
    online
        .entry(room.clone())
        .or_default()
        .insert(username.clone());

    // 推送历史消息
    match db::get_room_history(&state.db, &room, 50).await {
        Ok(messages) => {
            for msg in messages {
                let server_msg = ServerMessage {
                    msg_type: "history".into(),
                    username: msg.username,
                    content: msg.content,
                };
                if socket
                    .send(Message::Text(
                        serde_json::to_string(&server_msg).expect("serialize failed"),
                    ))
                    .await
                    .is_err()
                {
                    return;
                }
            }
        }
        Err(e) => {
            tracing::error!("查询历史消息失败: {}", e);
        }
    }

    broadcast_system(&tx, &room, &format!("用户 {} 加入房间 {}", username, room)).await;

    let mut rx = tx.subscribe();
    let mut heartbeat = interval(Duration::from_secs(30));
    heartbeat.tick().await;

    // === 主循环 ===
    loop {
        tokio::select! {
            _ = heartbeat.tick() => {
                if socket.send(Message::Ping(vec![].into())).await.is_err() {
                    info!("用户 {} 心跳超时，断开", username);
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

            msg = rx.recv() => {
                match msg {
                    Ok(client_msg) => {
                        if !forward_to_client(&mut socket, client_msg).await {
                            break;
                        }
                    }
                    Err(broadcast::error::RecvError::Lagged(n)) => {
                        tracing::warn!("用户 {} 消息 lagged，丢失 {} 条", username, n);
                        continue;
                    }
                    Err(broadcast::error::RecvError::Closed) => break,
                }
            }
        }
    }

    let mut online = state.online.write().await;
    if let Some(members) = online.get_mut(&room) {
        members.remove(&username);
    }
    broadcast_system(&tx, &room, &format!("{} 离开了房间", username)).await;
    info!("用户 {} 离开房间 {}", username, room);
}

async fn broadcast_system(tx: &broadcast::Sender<ClientMessage>, room: &str, content: &str) {
    if let Err(e) = tx.send(ClientMessage {
        username: "系统".into(),
        room: room.into(),
        content: content.into(),
    }) {
        tracing::error!("广播消息发送失败: {}", e);
    }
}

async fn send_error(socket: &mut WebSocket, content: &str) {
    let _ = socket
        .send(Message::Text(
            serde_json::to_string(&ServerMessage {
                msg_type: "error".into(),
                username: "".into(),
                content: content.into(),
            })
            .expect("serialize ServerMessage failed"),
        ))
        .await;
}

/// 解析 join 消息
async fn parse_join_message(socket: &mut WebSocket) -> Option<ClientMessage> {
    match socket.recv().await {
        Some(Ok(Message::Text(text))) => match serde_json::from_str::<ClientMessage>(&text) {
            Ok(msg) => Some(msg),
            Err(_) => {
                send_error(socket, "消息格式错误").await;
                None
            }
        },
        _ => None,
    }
}

/// 查找房间
async fn find_room(
    state: &AppState,
    room: &str,
    socket: &mut WebSocket,
) -> Option<broadcast::Sender<ClientMessage>> {
    let rooms = state.rooms.read().await;
    match rooms.get(room) {
        Some(tx) => Some(tx.clone()),
        None => {
            send_error(socket, &format!("房间「{}」不存在", room)).await;
            None
        }
    }
}

/// 处理单条客户端消息
async fn handle_client_message(state: &AppState, text: &str, username: &str, room: &str) {
    match serde_json::from_str::<ClientMessage>(text) {
        Ok(m) => {
            let tx = {
                let rooms = state.rooms.read().await;
                rooms.get(room).cloned()
            };
            if let Some(tx) = tx {
                if let Err(e) = tx.send(ClientMessage {
                    username: username.into(),
                    room: room.into(),
                    content: m.content.clone(),
                }) {
                    tracing::error!("广播消息发送失败: {}", e);
                } else {
                    if let Err(e) = db::save_message(&state.db, &username, &room, &m.content).await
                    {
                        tracing::error!("保存消息失败: {}", e);
                    }
                }
            }
        }
        Err(_) => {} // 格式错误的消息静默丢弃
    }
}
/// 将广播消息转发给客户端
async fn forward_to_client(socket: &mut WebSocket, client_msg: ClientMessage) -> bool {
    let server_msg = ServerMessage {
        msg_type: "message".into(),
        username: client_msg.username,
        content: client_msg.content,
    };
    match socket
        .send(Message::Text(
            serde_json::to_string(&server_msg).expect("serialize ServerMessage failed"),
        ))
        .await
    {
        Ok(_) => true,
        Err(_) => false, // 发送失败，断开连接
    }
}
