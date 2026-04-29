mod message;

use crate::auth::token::verify_token;
use crate::db;
use crate::models::ClientMessage;
use crate::models::{ServerMessage, UnreadItem, UnreadSummary};
use crate::state::AppState;
use crate::ws::message::{find_group_room, forward_to_client, handle_client_message, send_error};
use axum::extract::ws::{Message, WebSocket};
use std::time::Duration;
use tokio::sync::{broadcast, mpsc};
use tokio::time::interval;
use tracing::{info, warn};

// ── 会话状态 ────────────────────────────────────────────────────────────────

struct SessionCtx {
    current_group: Option<String>,
    group_rx: Option<broadcast::Receiver<ClientMessage>>,
    private_rx: Option<mpsc::Receiver<ClientMessage>>,
}

// ── 入口 ────────────────────────────────────────────────────────────────────

pub async fn handler_socket(mut socket: WebSocket, state: AppState, token: String) {
    // 1. 验证 token
    let username = match verify_token(&token) {
        Ok(name) => name,
        Err(_) => {
            send_error(&mut socket, "无效的 token, 请重新登录").await;
            return;
        }
    };

    // 2. 上线即创建私聊 channel，与 WS 连接绑定，全程不销毁
    let (private_tx, private_rx_init) = mpsc::channel::<ClientMessage>(64);
    state
        .private_rooms
        .write()
        .await
        .insert(username.clone(), private_tx);

    // 3. 初始化会话状态
    let mut ctx = SessionCtx {
        current_group: None,
        group_rx: None,
        private_rx: Some(private_rx_init),
    };

    // 4. 上线时推送未读摘要（基于 read_cursor 游标计算）
    deliver_unread_summary(&mut socket, &state, &username).await;

    // 5. 主循环
    let mut heartbeat = interval(Duration::from_secs(30));

    loop {
        if !run_once(&mut socket, &state, &username, &mut ctx, &mut heartbeat).await {
            break;
        }
    }

    // 6. 断线清理
    leave_group(&state, &username, &ctx.current_group).await;
    state.private_rooms.write().await.remove(&username);
    info!("用户 {} 下线", username);
}

// ── 主循环单次轮询 ───────────────────────────────────────────────────────────

async fn run_once(
    socket: &mut WebSocket,
    state: &AppState,
    username: &str,
    ctx: &mut SessionCtx,
    heartbeat: &mut tokio::time::Interval,
) -> bool {
    tokio::select! {
        _ = heartbeat.tick() => {
            handle_heartbeat(socket, username).await
        }
        msg = socket.recv() => {
            handle_socket_msg(socket, state, username, ctx, msg).await
        }
        msg = recv_group(&mut ctx.group_rx) => {
            handle_group_msg(socket, username, msg).await
        }
        msg = recv_private(&mut ctx.private_rx) => {
            handle_private_msg(socket, username, msg).await
        }
    }
}

// ── 心跳 ────────────────────────────────────────────────────────────────────

async fn handle_heartbeat(socket: &mut WebSocket, username: &str) -> bool {
    if socket.send(Message::Ping(vec![].into())).await.is_err() {
        info!("用户 {} 心跳超时", username);
        return false;
    }
    true
}

// ── 客户端发来消息 ───────────────────────────────────────────────────────────

async fn handle_socket_msg(
    socket: &mut WebSocket,
    state: &AppState,
    username: &str,
    ctx: &mut SessionCtx,
    msg: Option<Result<Message, axum::Error>>,
) -> bool {
    match msg {
        Some(Ok(Message::Text(text))) => match serde_json::from_str::<ClientMessage>(&text) {
            Ok(m) if m.msg_type == "switch" || m.msg_type == "switch_private" => {
                leave_group(state, username, &ctx.current_group).await;
                do_switch(
                    socket,
                    state,
                    username,
                    &m.msg_type,
                    &m.room,
                    &mut ctx.current_group,
                    &mut ctx.group_rx,
                )
                .await
            }
            Ok(_) => {
                let room_str = ctx.current_group.as_deref().unwrap_or("");
                handle_client_message(state, &text, username, room_str).await;
                true
            }
            Err(_) => true,
        },
        Some(Ok(Message::Pong(_))) => true,
        _ => {
            info!("用户 {} 断开连接", username);
            false
        }
    }
}

// ── 群聊消息接收与处理 ───────────────────────────────────────────────────────

async fn recv_group(
    group_rx: &mut Option<broadcast::Receiver<ClientMessage>>,
) -> Result<ClientMessage, broadcast::error::RecvError> {
    match group_rx.as_mut() {
        Some(rx) => rx.recv().await,
        None => std::future::pending().await,
    }
}

async fn handle_group_msg(
    socket: &mut WebSocket,
    username: &str,
    msg: Result<ClientMessage, broadcast::error::RecvError>,
) -> bool {
    match msg {
        Ok(client_msg) => forward_to_client(socket, client_msg).await,
        Err(broadcast::error::RecvError::Lagged(n)) => {
            warn!("用户 {} 丢失 {} 条群聊消息", username, n);
            true
        }
        Err(broadcast::error::RecvError::Closed) => false,
    }
}

// ── 私聊消息接收与处理 ───────────────────────────────────────────────────────

async fn recv_private(
    private_rx: &mut Option<mpsc::Receiver<ClientMessage>>,
) -> Option<ClientMessage> {
    match private_rx.as_mut() {
        Some(rx) => rx.recv().await,
        None => std::future::pending().await,
    }
}

async fn handle_private_msg(
    socket: &mut WebSocket,
    username: &str,
    msg: Option<ClientMessage>,
) -> bool {
    match msg {
        Some(client_msg) => forward_to_client(socket, client_msg).await,
        None => {
            warn!("用户 {} 私聊 channel 已关闭", username);
            false
        }
    }
}

// ── 切换会话 ────────────────────────────────────────────────────────────────

async fn do_switch(
    socket: &mut WebSocket,
    state: &AppState,
    username: &str,
    msg_type: &str,
    room: &str,
    current_group: &mut Option<String>,
    group_rx: &mut Option<broadcast::Receiver<ClientMessage>>,
) -> bool {
    if msg_type == "switch_private" {
        // 切换到私聊模式
        info!("用户 {} 切换到私聊模式", username);
        let conv_id = {
            let a = username.min(room);
            let b = username.max(room);
            format!("{}_{}", a, b)
        };
        // 推送该私聊会话的未读消息
        deliver_unread_messages(socket, state, username, "private", &conv_id).await;
        *group_rx = None;
        *current_group = None;
    } else {
        // 切换到群聊模式
        info!("用户 {} 切换到群聊 {}", username, room);

        let tx = match find_group_room(state, room, socket).await {
            Some(tx) => tx,
            None => return false,
        };

        state
            .online
            .write()
            .await
            .entry(room.to_string())
            .or_default()
            .insert(username.to_string());

        broadcast_system(&tx, room, &format!("用户 {} 加入房间", username)).await;

        // 推送该群的未读消息
        deliver_unread_messages(socket, state, username, "group", room).await;

        *group_rx = Some(tx.subscribe());
        *current_group = Some(room.to_string());
    }
    true
}

// ── 离开群组 ────────────────────────────────────────────────────────────────

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

// ── 上线未读摘要推送（基于 read_cursor）────────────────────────────────────
//
// 用户上线时，遍历所有群组和私聊会话，计算未读数，
// 汇总成一条 unread_summary 消息推给前端显示角标

async fn deliver_unread_summary(socket: &mut WebSocket, state: &AppState, username: &str) {
    let mut items: Vec<UnreadItem> = Vec::new();

    // ── 群聊未读 ──
    if let Ok(groups) = db::get_user_groups(&state.db, username).await {
        for g in groups {
            if let Ok(count) = db::get_unread_count(&state.db, username, "group", &g.group_id).await {
                if count > 0 {
                    items.push(UnreadItem {
                        item_type: "group".into(),
                        id: g.group_id,
                        count,
                    });
                }
            }
        }
    }

    // ── 私聊未读 ──
    if let Ok(convs) = db::get_user_conversations(&state.db, username).await {
        for conv in convs {
            if conv.conv_type != "private" { continue; }
            if let Ok(count) = db::get_unread_count(&state.db, username, "private", &conv.conv_id).await {
                if count > 0 {
                    // 从 conv_id 解析出对方 username
                    let peer = if conv.conv_id.starts_with(&format!("{}_", username)) {
                        conv.conv_id[username.len() + 1..].to_string()
                    } else {
                        conv.conv_id[..conv.conv_id.len() - username.len() - 1].to_string()
                    };
                    items.push(UnreadItem {
                        item_type: "private".into(),
                        id: peer,
                        count,
                    });
                }
            }
        }
    }

    if items.is_empty() { return; }

    let msg = UnreadSummary {
        msg_type: "unread_summary".into(),
        items,
    };

    let _ = socket
        .send(Message::Text(serde_json::to_string(&msg).unwrap()))
        .await;
}

// ── 切换会话时推送未读消息（基于 read_cursor）──────────────────────────────
//
// 用户打开某个会话时，从消息表中取出 last_read_id 之后的消息逐条推送，
// 推完后更新游标标记为已读

async fn deliver_unread_messages(
    socket: &mut WebSocket,
    state: &AppState,
    username: &str,
    session_type: &str,
    session_id: &str,
) {
    let msgs = match db::get_unread_messages(&state.db, username, session_type, session_id).await {
        Ok(m) => m,
        Err(e) => {
            warn!("拉取未读消息失败: {}", e);
            return;
        }
    };

    for msg in &msgs {
        let server_msg = ServerMessage {
            msg_type: msg.msg_type.clone(),
            username: msg.username.clone(),
            content: msg.content.clone(),
            message_id: msg.message_id,
            recalled: if msg.recalled { Some(true) } else { None },
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

    // 推完后更新游标
    if let Err(e) = db::mark_session_read(&state.db, username, session_type, session_id).await {
        warn!("更新已读游标失败: {}", e);
    }
}

// ── 系统广播 ────────────────────────────────────────────────────────────────

async fn broadcast_system(tx: &broadcast::Sender<ClientMessage>, room: &str, content: &str) {
    let _ = tx.send(ClientMessage {
        msg_type: "message".into(),
        username: "系统".into(),
        room: room.into(),
        content: content.into(),
        ..Default::default()
    });
}
