use super::session::TerminalSession;
use axum::extract::ws::{Message, WebSocket};
use futures_util::{SinkExt, StreamExt};
use serde::Deserialize;
use std::sync::Arc;
use tokio::sync::broadcast;

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum TerminalControl {
    Resize { cols: u16, rows: u16 },
}

/// 连接浏览器 WebSocket 与 PTY 输入输出。
///
/// 参数:
/// - `socket`: 浏览器 WebSocket
/// - `session`: PTY 会话
pub(crate) async fn serve_socket(socket: WebSocket, session: Arc<TerminalSession>) {
    let (mut sender, mut receiver) = socket.split();
    let mut output = session.subscribe();
    let replay = session.replay();
    if !replay.is_empty() {
        // 1. 先发送 replay_start 标记，提示前端进入回放阶段并抑制自动响应
        if sender
            .send(Message::Text(r#"{"type":"replay_start"}"#.into()))
            .await
            .is_err()
        {
            return;
        }
        // 2. 发送回放缓冲的历史输出
        if sender.send(Message::Binary(replay)).await.is_err() {
            return;
        }
        // 3. 发送 replay_end 标记，前端在解析完回放数据后恢复输入转发
        if sender
            .send(Message::Text(r#"{"type":"replay_end"}"#.into()))
            .await
            .is_err()
        {
            return;
        }
    }
    loop {
        tokio::select! {
            message = receiver.next() => {
                let Some(Ok(message)) = message else {
                    break;
                };
                match message {
                    Message::Binary(bytes) => {
                        if session.write(&bytes).is_err() {
                            break;
                        }
                    }
                    Message::Text(text) => {
                        if let Ok(TerminalControl::Resize { cols, rows }) = serde_json::from_str(&text) {
                            let _ = session.resize(cols, rows);
                        }
                    }
                    Message::Close(_) => break,
                    Message::Ping(bytes) => {
                        if sender.send(Message::Pong(bytes)).await.is_err() {
                            break;
                        }
                    }
                    Message::Pong(_) => {}
                }
            }
            chunk = output.recv() => {
                match chunk {
                    Ok(chunk) => {
                        if sender.send(Message::Binary(chunk)).await.is_err() {
                            break;
                        }
                    }
                    Err(broadcast::error::RecvError::Lagged(_)) => continue,
                    Err(broadcast::error::RecvError::Closed) => break,
                }
            }
        }
    }
}
