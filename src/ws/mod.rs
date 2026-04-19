mod message;

use crate::auth::token::verify_token;
use crate::db;
use crate::models::{ClientMessage, ServerMessage};
use crate::state::AppState;
use crate::ws::message::{find_group_room, forward_to_client, handle_client_message, parse_join_message, send_error};
use axum::extract::ws::{Message, WebSocket};
use std::time::Duration;
use tokio::sync::{broadcast, mpsc};
use tokio::time::interval;
use tracing::{info, warn};

pub async fn handler_socket(mut socket: WebSocket, state: AppState, token: String) {
    // 1. 验证 token
    let username = match verify_token(&token) {
        Ok(name) => name,
        Err(_) => {
            send_error(&mut socket, "无效的 token, 请重新登录").await;
            return;
        }
    };

    // 2. 等待客户端首条消息（决定初始会话类型）
    let join_msg = match parse_join_message(&mut socket).await {
        Some(msg) => msg,
        None => return,
    };

    // 3. 初始化会话状态
    // current_group: 当前所在群组 ID，None 表示私聊模式
    let mut current_group: Option<String> = None;
    // group_rx: 群聊广播接收端
    let mut group_rx: Option<broadcast::Receiver<ClientMessage>> = None;
    // private_rx: 私聊 mpsc 接收端（上线时创建，下线时 drop）
    let mut private_rx: Option<mpsc::Receiver<ClientMessage>> = None;

    // 4. 处理首条消息，进入初始会话
    if !do_switch(
        &mut socket, &state, &username,
        &join_msg.msg_type, &join_msg.room,
        &mut current_group, &mut group_rx, &mut private_rx,
    ).await {
        return;
    }

    // 5. 推送离线消息
    deliver_offline(&mut socket, &state, &username).await;

    // 6. 统一主循环
    let mut heartbeat = interval(Duration::from_secs(30));
    heartbeat.tick().await;

    loop {
        tokio::select! {
            // 心跳
            _ = heartbeat.tick() => {
                if socket.send(Message::Ping(vec![].into())).await.is_err() {
                    info!("用户 {} 心跳超时", username);
                    break;
                }
            }

            // 客户端发来消息
            msg = socket.recv() => {
                match msg {
                    Some(Ok(Message::Text(text))) => {
                        if let Ok(m) = serde_json::from_str::<ClientMessage>(&text) {
                            if m.msg_type == "switch" || m.msg_type == "switch_private" {
                                // 离开当前房间
                                leave_group(&state, &username, &current_group).await;
                                // 切换会话
                                if !do_switch(
                                    &mut socket, &state, &username,
                                    &m.msg_type, &m.room,
                                    &mut current_group, &mut group_rx, &mut private_rx,
                                ).await {
                                    break;
                                }
                            } else {
                                let room_str = current_group.as_deref().unwrap_or("");
                                handle_client_message(&state, &text, &username, room_str).await;
                            }
                        }
                    }
                    Some(Ok(Message::Pong(_))) => {}
                    _ => {
                        info!("用户 {} 断开连接", username);
                        break;
                    }
                }
            }

            // 收群聊广播消息
            msg = async {
                match group_rx.as_mut() {
                    Some(rx) => rx.recv().await,
                    None => std::future::pending().await,
                }
            } => {
                match msg {
                    Ok(client_msg) => {
                        if !forward_to_client(&mut socket, client_msg).await {
                            break;
                        }
                    }
                    Err(broadcast::error::RecvError::Lagged(n)) => {
                        warn!("用户 {} 丢失 {} 条群聊消息", username, n);
                    }
                    Err(broadcast::error::RecvError::Closed) => break,
                }
            }

            // 收私聊 mpsc 消息
            msg = async {
                match private_rx.as_mut() {
                    Some(rx) => rx.recv().await,
                    None => std::future::pending().await,
                }
            } => {
                match msg {
                    Some(client_msg) => {
                        if !forward_to_client(&mut socket, client_msg).await {
                            break;
                        }
                    }
                    // mpsc recv 返回 None 说明所有 Sender 都 drop 了
                    None => {
                        warn!("用户 {} 私聊 channel 已关闭", username);
                        break;
                    }
                }
            }
        }
    }

    // 7. 断线清理
    leave_group(&state, &username, &current_group).await;
    // 移除私聊 Sender，rx 随 loop 结束自动 drop
    state.private_rooms.write().await.remove(&username);
    info!("用户 {} 下线", username);
}

/// 切换会话（群聊 or 私聊），更新 current_group / group_rx / private_rx
async fn do_switch(
    socket: &mut WebSocket,
    state: &AppState,
    username: &str,
    msg_type: &str,
    room: &str,
    current_group: &mut Option<String>,
    group_rx: &mut Option<broadcast::Receiver<ClientMessage>>,
    private_rx: &mut Option<mpsc::Receiver<ClientMessage>>,
) -> bool {
    if msg_type == "switch_private" || msg_type == "private" {
        // 切换到私聊模式
        info!("用户 {} 切换到私聊模式", username);

        // 创建个人 mpsc channel，tx 存 private_rooms，rx 留在 loop
        let (tx, rx) = mpsc::channel::<ClientMessage>(64);
        state.private_rooms.write().await.insert(username.to_string(), tx);

        *private_rx = Some(rx);
        *group_rx = None;         // 不再订阅群聊
        *current_group = None;
    } else {
        // 切换到群聊模式
        info!("用户 {} 切换到群聊 {}", username, room);

        let tx = match find_group_room(state, room, socket).await {
            Some(tx) => tx,
            None => return false,
        };

        // 加入 online map
        state.online.write().await
            .entry(room.to_string())
            .or_default()
            .insert(username.to_string());

        broadcast_system(&tx, room, &format!("用户 {} 加入房间", username)).await;

        *group_rx = Some(tx.subscribe());
        *private_rx = None;       // 不再监听私聊
        *current_group = Some(room.to_string());

        // 切换到群聊后，从 private_rooms 移除（不再接收私聊实时推送）
        // 注意：私聊消息仍会存 offline_messages，上线再推
        state.private_rooms.write().await.remove(username);
    }
    true
}

/// 离开当前群组，清理 online map 并广播退出消息
async fn leave_group(state: &AppState, username: &str, current_group: &Option<String>) {
    if let Some(room) = current_group {
        {
            let mut online = state.online.write().await;
            if let Some(members) = online.get_mut(room) {
                members.remove(username);
            }
        }
        let tx = state.group_rooms.read().await.get(room).cloned();
        if let Some(tx) = tx {
            broadcast_system(&tx, room, &format!("{} 离开了房间", username)).await;
        }
        info!("用户 {} 离开群组 {}", username, room);
    }
}

/// 推送并清理离线消息
async fn deliver_offline(socket: &mut WebSocket, state: &AppState, username: &str) {
    if let Ok(offline) = db::get_offline_messages(&state.db, username).await {
        if !offline.is_empty() {
            for msg in &offline {
                let server_msg = ServerMessage {
                    msg_type: msg.msg_type.clone(),
                    username: msg.username.clone(),
                    content: format!("[离线消息] {}", msg.content),
                    ..Default::default()
                };
                if socket
                    .send(Message::Text(serde_json::to_string(&server_msg).unwrap()))
                    .await
                    .is_err()
                {
                    return;
                }
            }
            if let Err(e) = db::clear_offline_messages(&state.db, username).await {
                warn!("清理离线消息失败: {}", e);
            }
        }
    }
}

async fn broadcast_system(tx: &broadcast::Sender<ClientMessage>, room: &str, content: &str) {
    let _ = tx.send(ClientMessage {
        msg_type: "message".into(),
        username: "系统".into(),
        room: room.into(),
        content: content.into(),
        ..Default::default()
    });
}