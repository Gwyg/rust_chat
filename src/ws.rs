use std::time::Duration;
use axum::extract::ws::{Message, WebSocket};
use tokio::time::interval;
use tracing::info;
use crate::models::{ClientMessage, ServerMessage};
use crate::state::AppState;

pub async fn handler_socket(mut socket: WebSocket, state: AppState) {
    // 解析 join 消息
    let json_msg = match socket.recv().await {
        Some(Ok(Message::Text(text))) => {
            match serde_json::from_str::<ClientMessage>(&text) {
                Ok(msg) => msg,
                Err(_) => {
                    let _ = socket.send(Message::Text(
                        serde_json::to_string(&ServerMessage {
                            msg_type: "error".into(),
                            username: "".into(),
                            content: "消息格式错误".into(),
                        }).unwrap(),
                    )).await;
                    return;
                }
            }
        }
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
                let _ = socket.send(Message::Text(
                    serde_json::to_string(&ServerMessage {
                        msg_type: "error".into(),
                        username: "".into(),
                        content: format!("房间「{}」不存在", room),
                    }).unwrap(),
                )).await;
                return;
            }
        }
    };

    info!("用户 {} 加入房间 {}", username, room);

    let _ = tx.send(ClientMessage {
        username: "系统".into(),
        room: room.clone(),
        content: format!("用户 {} 加入房间 {}", username, room),
    });

    let mut rx = tx.subscribe();

    // 跳过第一个立即触发的 tick
    let mut heartbeat = interval(Duration::from_secs(30));
    heartbeat.tick().await;

    loop {
        tokio::select! {
            _ = heartbeat.tick() => {
                if socket.send(Message::Ping(vec![].into())).await.is_err() {
                    info!("用户 {} 心跳超时，断开", username);
                    let _ = tx.send(ClientMessage {
                        username: "系统".into(),
                        room: room.clone(),
                        content: format!("{} 离开了房间", username),
                    });
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
                        let _ = tx.send(ClientMessage {
                            username: "系统".into(),
                            room: room.clone(),
                            content: format!("{} 离开了房间", username),
                        });
                        break;
                    }
                }
            }
            Ok(client_msg) = rx.recv() => {
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
        }
    }
}