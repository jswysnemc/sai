use super::kind::TerminalKind;
use axum::extract::ws::{Message, WebSocket};
use futures_util::{SinkExt, StreamExt};
use serde::Deserialize;
use std::sync::Arc;
use tokio::sync::broadcast;

use super::replay::TerminalChunk;

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum TerminalControl {
    Resize { cols: u16, rows: u16 },
}

/// 连接浏览器 WebSocket 与终端输入输出。
///
/// 参数:
/// - `socket`: 浏览器 WebSocket
/// - `session`: 本地或 SSH 终端会话
pub(crate) async fn serve_socket(socket: WebSocket, session: Arc<TerminalKind>) {
    let (mut sender, mut receiver) = socket.split();
    let mut output = session.subscribe();
    // 先订阅再取快照：快照之后的分片留在通道里，已包含在快照中的分片按偏移跳过
    let (replay, mut cursor) = session.replay_since(0);
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
                        if session.write(&bytes).await.is_err() {
                            break;
                        }
                    }
                    Message::Text(text) => {
                        if let Ok(TerminalControl::Resize { cols, rows }) = serde_json::from_str(&text) {
                            let _ = session.resize(cols, rows).await;
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
                        if !forward_terminal_chunk(&mut sender, &session, &mut cursor, &chunk).await {
                            break;
                        }
                    }
                    // 通道溢出时分片已丢，按偏移从回放缓冲补回，避免屏幕随机少字
                    Err(broadcast::error::RecvError::Lagged(_)) => {
                        if !forward_replay_gap(&mut sender, &session, &mut cursor).await {
                            break;
                        }
                    }
                    Err(broadcast::error::RecvError::Closed) => break,
                }
            }
        }
    }
}

/// 把一个输出分片写给浏览器，缺口先从回放缓冲补齐。
///
/// 参数:
/// - `sender`: WebSocket 发送端
/// - `session`: 终端会话
/// - `cursor`: 已经写给浏览器的字节偏移
/// - `chunk`: 新到达的分片
///
/// 返回:
/// - 连接是否仍然可用
async fn forward_terminal_chunk(
    sender: &mut futures_util::stream::SplitSink<WebSocket, Message>,
    session: &TerminalKind,
    cursor: &mut u64,
    chunk: &TerminalChunk,
) -> bool {
    // 1. 分片起点越过游标时，中间字节已经从广播通道丢掉
    if chunk.start > *cursor && !forward_replay_gap(sender, session, cursor).await {
        return false;
    }
    // 2. 回放已经覆盖到本分片末尾时不再重复发送
    let end = chunk.start.saturating_add(chunk.bytes.len() as u64);
    if end <= *cursor {
        return true;
    }
    let skip = usize::try_from(cursor.saturating_sub(chunk.start)).unwrap_or(chunk.bytes.len());
    let unseen = &chunk.bytes[skip.min(chunk.bytes.len())..];
    if unseen.is_empty() {
        return true;
    }
    if sender
        .send(Message::Binary(unseen.to_vec()))
        .await
        .is_err()
    {
        return false;
    }
    *cursor = end;
    true
}

/// 按当前游标把回放缓冲里还没写给浏览器的字节补上。
///
/// 参数:
/// - `sender`: WebSocket 发送端
/// - `session`: 终端会话
/// - `cursor`: 已经写给浏览器的字节偏移
///
/// 返回:
/// - 连接是否仍然可用
async fn forward_replay_gap(
    sender: &mut futures_util::stream::SplitSink<WebSocket, Message>,
    session: &TerminalKind,
    cursor: &mut u64,
) -> bool {
    let (gap, total) = session.replay_since(*cursor);
    if !gap.is_empty()
        && sender
            .send(Message::Binary(gap))
            .await
            .is_err()
    {
        return false;
    }
    *cursor = total;
    true
}
