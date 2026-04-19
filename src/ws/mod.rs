mod message;

use crate::auth::token::verify_token;
use crate::db;
use crate::models::{ClientMessage, ServerMessage};
use crate::state::AppState;
use crate::ws::message::{find_room, forward_to_client, handle_client_message, parse_join_message};
use axum::extract::ws::{Message, WebSocket};
use std::time::Duration;
use tokio::sync::broadcast;
use tokio::time::interval;
use tracing::{info, warn};

pub async fn handler_socket(mut socket: WebSocket, state: AppState, token: String) {
    // 1. 验证 token
    let username = match verify_token(&token) {
        Ok(name) => name,
        Err(_) => {
            message::send_error(&mut socket, "无效的 token, 请重新登录").await;
            return;
        }
    };

    // 2. 等待客户端的首条 join 消息
    let join_msg = match parse_join_message(&mut socket).await {
        Some(msg) => msg,
        None => return,
    };

    // 3. 根据 msg_type 分流：私聊模式 or 群聊模式
    if join_msg.msg_type == "private" {
        // 私聊不需要 join 房间，直接进入只处理消息的轻量 loop
        run_private_loop(&mut socket, &state, &username).await;
    } else {
        // 群聊模式：必须找到对应 room
        run_group_loop(&mut socket, &state, &username, &join_msg.room).await;
    }
}

/// 群聊 loop：加入房间、推送历史、广播收发
async fn run_group_loop(socket: &mut WebSocket, state: &AppState, username: &str, room: &str) {
    let tx = match find_room(state, room, socket).await {
        Some(tx) => tx,
        None => return, // find_room 内部已发送错误消息
    };

    info!("用户 {} 加入群组 {}", username, room);
    state
        .online
        .write()
        .await
        .entry(room.to_string())
        .or_default()
        .insert(username.to_string());

    if let Ok(offline) = db::get_offline_messages(&state.db, &username).await {
        if !offline.is_empty() {
            for msg in &offline {
                let server_msg = ServerMessage {
                    msg_type: msg.msg_type.clone(),
                    username: msg.username.clone(),
                    content: format!("[离线消息] {}", msg.content),
                };
                if socket
                    .send(Message::Text(serde_json::to_string(&server_msg).unwrap()))
                    .await
                    .is_err()
                {
                    return;
                }
            }
            if let Err(e) = db::clear_offline_messages(&state.db, &username).await {
                warn!("清理离线消息失败: {}", e);
            }
        }
    }

    broadcast_system(&tx, room, &format!("用户 {} 加入房间", username)).await;

    let mut rx = tx.subscribe();
    let mut heartbeat = interval(Duration::from_secs(30));
    heartbeat.tick().await;

    loop {
        tokio::select! {
            _ = heartbeat.tick() => {
                if socket.send(Message::Ping(vec![].into())).await.is_err() {
                    info!("用户 {} 心跳超时", username);
                    break;
                }
            }
            msg = socket.recv() => {
                match msg {
                    Some(Ok(Message::Text(text))) => {
                        handle_client_message(state, &text, username, room).await;
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
                    if !forward_to_client(socket, client_msg).await {
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

    cleanup_group(state, room, username, &tx).await;
}

/// 私聊 loop：不订阅广播，只处理收发消息（private channel 在 handle_client_message 内动态创建）
async fn run_private_loop(socket: &mut WebSocket, state: &AppState, username: &str) {
    info!("用户 {} 进入私聊模式", username);

    // 订阅一个以自己用户名命名的个人 channel，用于接收别人发来的私聊
    let rx_tx = {
        let mut rooms = state.rooms.write().await;
        rooms
            .entry(format!("__private_{}", username))
            .or_insert_with(|| {
                let (tx, _) = broadcast::channel(64);
                tx
            })
            .clone()
    };
    let mut rx = rx_tx.subscribe();
    let mut heartbeat = interval(Duration::from_secs(30));
    heartbeat.tick().await;

    loop {
        tokio::select! {
            _ = heartbeat.tick() => {
                if socket.send(Message::Ping(vec![].into())).await.is_err() {
                    info!("用户 {} 私聊心跳超时", username);
                    break;
                }
            }
            msg = socket.recv() => {
                match msg {
                    Some(Ok(Message::Text(text))) => {
                        // 私聊消息交给统一 handler 处理（msg_type == "private"）
                        handle_client_message(state, &text, username, "").await;
                    }
                    Some(Ok(Message::Pong(_))) => {}
                    _ => {
                        info!("用户 {} 私聊断开", username);
                        break;
                    }
                }
            }
            // 接收其他人发来的私聊消息
            msg = rx.recv() => match msg {
                Ok(client_msg) => {
                    if !forward_to_client(socket, client_msg).await {
                        break;
                    }
                }
                Err(broadcast::error::RecvError::Lagged(n)) => {
                    warn!("用户 {} 私聊丢失 {} 条消息", username, n);
                    continue;
                }
                Err(broadcast::error::RecvError::Closed) => break,
            },
        }
    }

    // 退出时清理个人 channel
    state
        .rooms
        .write()
        .await
        .remove(&format!("__private_{}", username));
    info!("用户 {} 离开私聊模式", username);
}

/// 群聊退出清理
async fn cleanup_group(
    state: &AppState,
    room: &str,
    username: &str,
    tx: &broadcast::Sender<ClientMessage>,
) {
    let mut online = state.online.write().await;
    if let Some(members) = online.get_mut(room) {
        members.remove(username);
    }
    drop(online);
    broadcast_system(tx, room, &format!("{} 离开了房间", username)).await;
    info!("用户 {} 离开群组 {}", username, room);
}

async fn broadcast_system(tx: &broadcast::Sender<ClientMessage>, room: &str, content: &str) {
    let _ = tx.send(ClientMessage {
        username: "系统".into(),
        room: room.into(),
        content: content.into(),
        msg_type: "message".into(),
    });
}
