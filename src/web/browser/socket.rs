//! 浏览器面板连接：独立接收输入、执行命令与发送最新画面。

use super::commands;
use super::frame_delivery::{self, text};
use super::input_queue::InputQueue;
use super::protocol::{ClientMessage, ServerMessage};
use crate::browser::BrowserEvent;
use crate::web::server_logging;
use axum::extract::ws::{Message, WebSocket};
use futures_util::{SinkExt, StreamExt};
use std::sync::Arc;
use tokio::sync::{broadcast, mpsc, watch};

/// 【浏览器面板】【连接服务】启动或复用共享浏览器，并管理连接期间的收发任务。
/// @param socket 为面板 WebSocket
/// @returns 无；断开时取消输入和导航任务并注销录屏订阅
pub(crate) async fn serve_socket(socket: WebSocket) {
    let (mut sender, mut receiver) = socket.split();
    // 1. 【浏览器面板】【连接服务】获取共享会话；浏览器缺失或启动失败时返回原因
    let session = match crate::browser::shared().await {
        Ok(session) => session,
        Err(error) => {
            let message = format!("{error:#}");
            server_logging::write("浏览器启动失败", &message, true);
            let _ = sender
                .send(text(&ServerMessage::Error { message: &message }))
                .await;
            let _ = sender.close().await;
            return;
        }
    };
    server_logging::write("浏览器面板", "面板已连接", false);
    let mut events = session.subscribe();
    session.attach_viewer().await;
    let state = session
        .current_state()
        .await
        .unwrap_or_else(|_| session.last_state());

    // 2. 【浏览器面板】【任务分离】写端独立执行；画面槽位覆盖旧帧，输入执行期间仍可接收新事件
    let (frames_tx, frames_rx) = watch::channel(None);
    let (outgoing, messages) = mpsc::unbounded_channel();
    let _ = outgoing.send(text(&ServerMessage::State { state: &state }));
    let mut delivery = tokio::spawn(frame_delivery::deliver(sender, frames_rx, messages));
    let queue = Arc::new(InputQueue::default());
    let mut input = tokio::spawn(commands::run(
        session.clone(),
        queue.clone(),
        outgoing.clone(),
    ));

    // 3. 【浏览器面板】【消息接收】收集输入和浏览器事件，不在接收循环中等待 CDP 或网络写入
    loop {
        tokio::select! {
            _ = &mut delivery => break,
            _ = &mut input => break,
            message = receiver.next() => {
                let Some(Ok(message)) = message else { break };
                match message {
                    Message::Text(raw) => match serde_json::from_str::<ClientMessage>(&raw) {
                        Ok(message) => {
                            if let Err(error) = queue.push(message) {
                                server_logging::write("浏览器面板", error, true);
                                break;
                            }
                        }
                        Err(error) => {
                            let message = format!("invalid browser message: {error}");
                            let _ = outgoing.send(text(&ServerMessage::Error { message: &message }));
                        }
                    },
                    Message::Close(_) => break,
                    Message::Ping(bytes) => { let _ = outgoing.send(Message::Pong(bytes)); }
                    Message::Binary(_) | Message::Pong(_) => {}
                }
            }
            event = events.recv() => match event {
                Ok(BrowserEvent::Frame(bytes)) => { frames_tx.send_replace(Some(bytes)); }
                Ok(BrowserEvent::State(state)) => {
                    let _ = outgoing.send(text(&ServerMessage::State { state: &state }));
                }
                Ok(BrowserEvent::Activity(message)) => {
                    let _ = outgoing.send(text(&ServerMessage::Activity { message: &message }));
                }
                Err(broadcast::error::RecvError::Lagged(_)) => continue,
                Err(broadcast::error::RecvError::Closed) => break,
            }
        }
    }
    // 4. 【浏览器面板】【断开清理】取消当前连接的任务，避免断开后继续处理积压操作
    delivery.abort();
    input.abort();
    session.detach_viewer().await;
    server_logging::write("浏览器面板", "面板已断开", false);
}
