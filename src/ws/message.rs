use crate::db;
use crate::models::{ClientMessage, ServerMessage};
use crate::state::AppState;
use axum::extract::ws::{Message, WebSocket};
use tokio::sync::broadcast;
use tracing::error;

pub async fn handle_client_message(state: &AppState, text: &str, username: &str, room: &str) {
    match serde_json::from_str::<ClientMessage>(text) {
        Ok(m) => match m.msg_type.as_str() {
            "message" => {
                let tx = {
                    let rooms = state.rooms.read().await;
                    rooms.get(room).cloned()
                };

                if let Some(tx) = tx {
                    if let Err(e) = tx.send(ClientMessage {
                        msg_type: "message".into(),
                        username: username.into(),
                        room: room.into(),
                        content: m.content.clone(),
                    }) {
                        error!("广播消息失败: {}", e);
                    } else if let Err(e) =
                        db::save_message(&state.db, username, room, &m.content).await
                    {
                        error!("保存消息失败: {}", e);
                    }
                }
                // 在广播+保存群聊消息成功后，追加离线消息逻辑：
                // 对群内不在线的成员推送离线消息
                let members = {
                    let online = state.online.read().await;
                    online.get(room).cloned().unwrap_or_default()
                };
                if let Ok(all_users) = db::get_group_members(&state.db, room).await {
                    for member in &all_users {
                        if member.username != username && !members.contains(&member.username) {
                            let _ = db::save_offline_message(
                                &state.db,
                                username,
                                &member.username,
                                &m.content,
                                "group", // msg_type
                                room,    // source_id = group_id
                            )
                            .await;
                        }
                    }
                }
            }
            "private" => {
                let target = m.room.clone();
                // conv_id 用于数据库存储，不用于 broadcast
                let conv_id = format!("{}_{}", username.min(&target), username.max(&target));

                // 发送方往接收方的个人频道发消息（与 run_private_loop 中的订阅对齐）
                let private_channel = format!("__private_{}", target);
                let tx = {
                    let mut rooms = state.rooms.write().await;
                    rooms
                        .entry(private_channel.clone())
                        .or_insert_with(|| broadcast::channel(64).0)
                        .clone()
                };

                let _ = tx.send(ClientMessage {
                    msg_type: "private".into(),
                    username: username.into(),
                    room: conv_id.clone(),
                    content: m.content.clone(),
                });

                if let Err(e) =
                    // 改为：
                    db::save_offline_message(
                        &state.db, username, &target, &m.content, "private", // msg_type
                        &conv_id,  // source_id = conv_id
                    )
                    .await
                {
                    error!("保存私聊消息失败: {}", e);
                }

                // 如果目标用户不在线，保存为离线消息
                let target_online = is_user_online(state, &target).await;
                if !target_online {
                    if let Err(e) =
                        // 改为：
                        db::save_offline_message(
                            &state.db, username, &target, &m.content, "private", // msg_type
                            &conv_id,  // source_id = conv_id
                        )
                        .await
                    {
                        error!("保存离线消息失败: {}", e);
                    }
                }
            }
            _ => {}
        },
        Err(_) => {}
    }
}

/// 检查用户是否在任意房间在线
async fn is_user_online(state: &AppState, username: &str) -> bool {
    let online = state.online.read().await;
    online.values().any(|members| members.contains(username))
}

pub async fn forward_to_client(socket: &mut WebSocket, client_msg: ClientMessage) -> bool {
    let server_msg = ServerMessage {
        msg_type: client_msg.msg_type,
        username: client_msg.username,
        content: client_msg.content,
    };
    socket
        .send(Message::Text(
            serde_json::to_string(&server_msg).expect("serialize failed"),
        ))
        .await
        .is_ok()
}

pub async fn parse_join_message(socket: &mut WebSocket) -> Option<ClientMessage> {
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

pub async fn find_room(
    state: &AppState,
    room: &str,
    socket: &mut WebSocket,
) -> Option<tokio::sync::broadcast::Sender<ClientMessage>> {
    let rooms = state.rooms.read().await;
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
            })
            .expect("serialize failed"),
        ))
        .await;
}
