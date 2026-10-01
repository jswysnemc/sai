//! 独立发送画面与控制消息；网络阻塞时仅保留最新的一帧。

use super::protocol::ServerMessage;
use axum::extract::ws::Message;
use futures_util::{Sink, SinkExt};
use std::sync::Arc;
use tokio::sync::{mpsc, watch};

pub(super) type Frame = Arc<Vec<u8>>;

/// 【浏览器面板】【画面发送】消费最新画面与文本消息，不占用输入接收任务。
/// @param sender 为 WebSocket 写端；frames 为最新画面；messages 为控制消息
/// @returns 连接关闭或发送失败时结束
pub(super) async fn deliver<S: Sink<Message> + Unpin>(
    mut sender: S,
    mut frames: watch::Receiver<Option<Frame>>,
    mut messages: mpsc::UnboundedReceiver<Message>,
) {
    loop {
        let outgoing = tokio::select! {
            // 【浏览器面板】【画面发送】优先发送状态与错误，避免控制消息排在连续画面之后
            biased;
            message = messages.recv() => {
                let Some(message) = message else { break };
                message
            }
            changed = frames.changed() => {
                if changed.is_err() { break; }
                let frame = frames.borrow_and_update().clone();
                let Some(frame) = frame else { continue };
                Message::Binary(frame.as_ref().clone())
            }
        };
        if sender.send(outgoing).await.is_err() {
            break;
        }
    }
}

/// 【浏览器面板】【文本消息】把状态、活动或错误转换为 WebSocket 消息。
/// @param message 为服务端消息
/// @returns 序列化后的文本帧
pub(super) fn text(message: &ServerMessage<'_>) -> Message {
    Message::Text(serde_json::to_string(message).unwrap_or_else(|_| "{}".to_string()))
}
