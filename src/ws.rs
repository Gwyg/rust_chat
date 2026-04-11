use crate::models::{ClientMessage, ServerMessage};
use crate::state::AppState;
use axum::extract::ws::{Message, WebSocket};
use std::time::Duration;
use tokio::sync::broadcast;
use tokio::time::interval;
use tracing::info;

pub async fn handler_socket(mut socket: WebSocket, state: AppState) {
    // 解析 join 消息
    let json_msg = match socket.recv().await {
        Some(Ok(Message::Text(text))) => match serde_json::from_str::<ClientMessage>(&text) {
            Ok(msg) => msg,
            Err(_) => {
                send_error(&mut socket, "消息格式错误").await;
                return;
            }
        },
        _ => return,
    };

    let username = json_msg.username;
    let room = json_msg.room;

    // 检查房间是否存在
    let tx = {
        let rooms = state.rooms.read().await;
        match rooms.get(&room) {
            Some(tx) => tx.clone(),
            None => {
                send_error(&mut socket, &format!("房间「{}」不存在", room)).await;
                return;
            }
        }
    };

    info!("用户 {} 加入房间 {}", username, room);

    if let Err(e) = tx.send(ClientMessage {
        username: "系统".into(),
        room: room.clone(),
        content: format!("用户 {} 加入房间 {}", username, room),
    }) {
        tracing::error!("广播消息发送失败: {}", e);
    }

    let mut rx = tx.subscribe();

    // 跳过第一个立即触发的 tick
    let mut heartbeat = interval(Duration::from_secs(30));
    heartbeat.tick().await;

    loop {
        tokio::select! {
            _ = heartbeat.tick() => {
                if socket.send(Message::Ping(vec![].into())).await.is_err() {
                    info!("用户 {} 心跳超时，断开", username);
                    if let Err(e) = tx.send(ClientMessage {
                        username: "系统".into(),
                        room: room.clone(),
                        content: format!("{} 离开了房间", username),
                    }) {
                        tracing::error!("广播消息发送失败: {}", e);
                    }
                    break;
                }
            }
            msg = socket.recv() => {
                match msg {
                    Some(Ok(Message::Text(text))) => {
                        let client_msg = match serde_json::from_str::<ClientMessage>(&text) {
                            Ok(m) => m,
                            Err(_) => continue,
                        };
                        let _ = tx.send(ClientMessage {
                            username: username.clone(),
                            room: room.clone(),
                            content: client_msg.content,
                        });
                    }
                    Some(Ok(Message::Pong(_))) => {}
                    _ => {
                        info!("用户 {} 断开", username);
                        if let Err(e) = tx.send(ClientMessage {
                            username: "系统".into(),
                            room: room.clone(),
                            content: format!("{} 离开了房间", username),
                        }) {
                            tracing::error!("广播消息发送失败: {}", e);
                        }
                        break;
                    }
                }
            }
            msg = rx.recv() => {
                match msg {
                    Ok(client_msg) => {
                        let server_msg = ServerMessage {
                            msg_type: "message".into(),
                            username: client_msg.username,
                            content: client_msg.content,
                        };
                        if socket.send(Message::Text(
                            serde_json::to_string(&server_msg).unwrap(),
                        )).await.is_err() {
                            break;
                        }

                    }
                    Err(broadcast::error::RecvError::Lagged(n)) => {
                        tracing::warn!("Lagged by {} messages", n);
                        continue;
                    }
                    Err(broadcast::error::RecvError::Closed) => break,
                }
            }
        }
    }
}

async fn send_error(socket: &mut WebSocket, content: &str) {
    let _ = socket.send(Message::Text(
        serde_json::to_string(&ServerMessage {
            msg_type: "error".into(),
            username: "".into(),
            content: content.into(),
        }).expect("serialize ServerMessage failed"),
    )).await;
}
