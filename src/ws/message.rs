use crate::db;
use crate::models::{ClientMessage, ServerMessage};
use crate::state::AppState;
use axum::extract::ws::{Message, WebSocket};
use tracing::error;

pub async fn handle_client_message(state: &AppState, text: &str, username: &str, room: &str) {
    let m = match serde_json::from_str::<ClientMessage>(text) {
        Ok(m) => m,
        Err(_) => return,
    };

    match m.msg_type.as_str() {
        // ── 群聊文字消息 ────────────────────────────
        "message" => {
            let tx = {
                let rooms = state.group_rooms.read().await;
                rooms.get(room).cloned()
            };

            if let Some(tx) = tx {
                if let Err(e) = tx.send(ClientMessage {
                    msg_type: "message".into(),
                    username: username.into(),
                    room: room.into(),
                    content: m.content.clone(),
                    ..Default::default()
                }) {
                    error!("广播群聊消息失败: {}", e);
                } else if let Err(e) = db::save_message(&state.db, username, room, &m.content).await {
                    error!("保存群聊消息失败: {}", e);
                }
            }

            // 对不在线的群成员存离线消息
            let online_members = {
                state.online.read().await.get(room).cloned().unwrap_or_default()
            };
            if let Ok(all_members) = db::get_group_members(&state.db, room).await {
                for member in &all_members {
                    if member.username != username && !online_members.contains(&member.username) {
                        let _ = db::save_offline_message(
                            &state.db, username, &member.username,
                            &m.content, "group", room,
                        ).await;
                    }
                }
            }
        }

        // ── 私聊文字消息 ────────────────────────────
        "private" => {
            let target = m.room.clone();
            let conv_id = format!("{}_{}", username.min(&target), username.max(&target));

            // 查 private_rooms 判断对方是否在线
            let tx = state.private_rooms.read().await.get(&target).cloned();

            if let Some(tx) = tx {
                // 对方在线，直接通过 mpsc 推送
                let _ = tx.send(ClientMessage {
                    msg_type: "private".into(),
                    username: username.into(),
                    room: conv_id.clone(),
                    content: m.content.clone(),
                    ..Default::default()
                }).await;
            } else {
                // 对方不在线，存离线消息
                if let Err(e) = db::save_offline_message(
                    &state.db, username, &target,
                    &m.content, "private", &conv_id,
                ).await {
                    error!("保存离线消息失败: {}", e);
                }
            }

            // 无论对方在不在线，都保存到 private_messages 表
            if let Err(e) = db::save_private_message(&state.db, username, &conv_id, &m.content).await {
                error!("保存私聊消息失败: {}", e);
            }
        }

        // ── 文件消息（群聊 or 私聊）────────────────────────────
        "file" => {
            let file_id = match &m.file_id {
                Some(id) => id.clone(),
                None => return,
            };
            let file_name = m.filename.clone().unwrap_or_default();
            let mime_type = m.mime_type.clone().unwrap_or_default();

            if room.is_empty() {
                // 私聊文件
                let target = m.room.clone();
                let conv_id = format!("{}_{}", username.min(&target), username.max(&target));

                let tx = state.private_rooms.read().await.get(&target).cloned();
                if let Some(tx) = tx {
                    let _ = tx.send(ClientMessage {
                        msg_type: "file".into(),
                        username: username.into(),
                        room: conv_id.clone(),
                        content: file_name.clone(),
                        file_id: Some(file_id.clone()),
                        filename: Some(file_name.clone()),
                        mime_type: Some(mime_type),
                        ..Default::default()
                    }).await;
                } else {
                    let _ = db::save_offline_message(
                        &state.db, username, &target,
                        &format!("[文件] {}", file_name),
                        "private", &conv_id,
                    ).await;
                }
            } else {
                // 群聊文件
                let tx = state.group_rooms.read().await.get(room).cloned();
                if let Some(tx) = tx {
                    let _ = tx.send(ClientMessage {
                        msg_type: "file".into(),
                        username: username.into(),
                        room: room.into(),
                        content: file_name.clone(),
                        file_id: Some(file_id.clone()),
                        filename: Some(file_name.clone()),
                        mime_type: Some(mime_type),
                        ..Default::default()
                    });

                    let online_members = {
                        state.online.read().await.get(room).cloned().unwrap_or_default()
                    };
                    if let Ok(all_members) = db::get_group_members(&state.db, room).await {
                        for member in &all_members {
                            if member.username != username && !online_members.contains(&member.username) {
                                let _ = db::save_offline_message(
                                    &state.db, username, &member.username,
                                    &format!("[文件] {}", file_name),
                                    "group", room,
                                ).await;
                            }
                        }
                    }
                }
            }
        }

        _ => {}
    }
}

pub async fn forward_to_client(socket: &mut WebSocket, client_msg: ClientMessage) -> bool {
    let server_msg = ServerMessage {
        msg_type: client_msg.msg_type,
        username: client_msg.username,
        content: client_msg.content,
        file_id: client_msg.file_id,
        filename: client_msg.filename,
        mime_type: client_msg.mime_type,
        message_id: client_msg.message_id,
        recalled: if client_msg.recalled { Some(true) } else { None },
    };
    socket
        .send(Message::Text(
            serde_json::to_string(&server_msg).expect("serialize failed"),
        ))
        .await
        .is_ok()
}

/// 从 group_rooms 查找群聊 channel
pub async fn find_group_room(
    state: &AppState,
    room: &str,
    socket: &mut WebSocket,
) -> Option<tokio::sync::broadcast::Sender<ClientMessage>> {
    let rooms = state.group_rooms.read().await;
    match rooms.get(room) {
        Some(tx) => Some(tx.clone()),
        None => {
            send_error(socket, &format!("房间「{}」不存在", room)).await;
            None
        }
    }
}

pub async fn send_error(socket: &mut WebSocket, content: &str) {
    let _ = socket
        .send(Message::Text(
            serde_json::to_string(&ServerMessage {
                msg_type: "error".into(),
                username: "".into(),
                content: content.into(),
                ..Default::default()
            })
            .expect("serialize failed"),
        ))
        .await;
}