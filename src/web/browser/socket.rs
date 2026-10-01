//! 浏览器面板 WebSocket：推送画面与状态，转发用户输入与导航操作。

use super::protocol::{ClientMessage, ServerMessage};
use crate::browser::{normalize_url, BrowserEvent, BrowserSession};
use crate::web::server_logging;
use anyhow::Result;
use axum::extract::ws::{Message, WebSocket};
use futures_util::{SinkExt, StreamExt};
use std::sync::Arc;
use tokio::sync::{broadcast, mpsc};

/// 【浏览器面板】【连接服务】启动或复用共享浏览器，并在连接期间双向转发。
/// @param socket 为面板 WebSocket
/// @returns 无
pub(crate) async fn serve_socket(socket: WebSocket) {
    let (mut sender, mut receiver) = socket.split();
    // 1. 获取共享会话；浏览器缺失或启动失败时把原因告诉面板后关闭
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
    // 2. 先订阅再登记面板，保证首帧截图与当前状态不会漏掉
    let mut events = session.subscribe();
    let (errors_tx, mut errors_rx) = mpsc::unbounded_channel::<String>();
    session.attach_viewer().await;
    let state = match session.current_state().await {
        Ok(state) => state,
        Err(_) => session.last_state(),
    };
    if sender
        .send(text(&ServerMessage::State { state: &state }))
        .await
        .is_err()
    {
        session.detach_viewer().await;
        return;
    }
    // 3. 主循环：面板消息、会话事件与异步操作错误三路并发
    loop {
        tokio::select! {
            message = receiver.next() => {
                let Some(Ok(message)) = message else { break };
                match message {
                    Message::Text(raw) => handle_client(&session, &raw, &errors_tx).await,
                    Message::Close(_) => break,
                    Message::Ping(bytes) => {
                        if sender.send(Message::Pong(bytes)).await.is_err() { break; }
                    }
                    Message::Binary(_) | Message::Pong(_) => {}
                }
            }
            event = events.recv() => {
                let outgoing = match event {
                    Ok(BrowserEvent::Frame(bytes)) => Message::Binary(bytes.as_ref().clone()),
                    Ok(BrowserEvent::State(state)) => text(&ServerMessage::State { state: &state }),
                    Ok(BrowserEvent::Activity(message)) => text(&ServerMessage::Activity { message: &message }),
                    // 网络慢时丢弃积压的旧画面，下一帧会覆盖
                    Err(broadcast::error::RecvError::Lagged(_)) => continue,
                    Err(broadcast::error::RecvError::Closed) => break,
                };
                if sender.send(outgoing).await.is_err() { break; }
            }
            Some(message) = errors_rx.recv() => {
                if sender.send(text(&ServerMessage::Error { message: &message })).await.is_err() { break; }
            }
        }
    }
    // 4. 面板断开后注销，最后一个面板离开时停止录屏
    session.detach_viewer().await;
    server_logging::write("浏览器面板", "面板已断开", false);
}

/// 【浏览器面板】【消息处理】解析并执行一条面板消息，导航类操作放到后台任务。
/// @param session 为浏览器会话；raw 为消息文本；errors 为错误回传通道
/// @returns 无
async fn handle_client(
    session: &Arc<BrowserSession>,
    raw: &str,
    errors: &mpsc::UnboundedSender<String>,
) {
    let message = match serde_json::from_str::<ClientMessage>(raw) {
        Ok(message) => message,
        Err(error) => {
            let _ = errors.send(format!("invalid browser message: {error}"));
            return;
        }
    };
    if message.is_slow() {
        let session = session.clone();
        let errors = errors.clone();
        tokio::spawn(async move {
            if let Err(error) = execute(&session, message).await {
                let _ = errors.send(format!("{error:#}"));
            }
        });
        return;
    }
    // 输入事件按顺序同步执行，保证按下与抬起的先后关系
    if let Err(error) = execute(session, message).await {
        let _ = errors.send(format!("{error:#}"));
    }
}

/// 【浏览器面板】【操作执行】把面板消息映射到浏览器会话操作。
/// @param session 为浏览器会话；message 为面板消息
/// @returns 操作结果
async fn execute(session: &BrowserSession, message: ClientMessage) -> Result<()> {
    match message {
        ClientMessage::Navigate { url } => {
            // 地址栏允许输入搜索词，协议限制与 Agent 工具一致
            let url = normalize_url(&url, true)?;
            session.navigate(&url).await?;
        }
        ClientMessage::Back => {
            session
                .go_history(crate::browser::navigation::HistoryStep::Back)
                .await?;
        }
        ClientMessage::Forward => {
            session
                .go_history(crate::browser::navigation::HistoryStep::Forward)
                .await?;
        }
        ClientMessage::Reload => {
            session.reload().await?;
        }
        ClientMessage::Stop => session.stop_loading().await?,
        ClientMessage::Mouse(input) => session.dispatch_mouse_input(&input).await?,
        ClientMessage::Key(input) => session.dispatch_key_input(&input).await?,
        ClientMessage::InsertText { text } => session.insert_text(&text).await?,
        ClientMessage::Resize { width, height } => session.resize(width, height).await?,
        ClientMessage::NewTab => {
            session.new_tab("about:blank").await?;
        }
        ClientMessage::SwitchTab { id } => session.switch_tab(&id).await?,
        ClientMessage::CloseTab { id } => session.close_tab(Some(&id)).await?,
    }
    Ok(())
}

/// 【浏览器面板】【文本消息】把服务端消息序列化为 WebSocket 文本帧。
/// @param message 为服务端消息
/// @returns WebSocket 消息
fn text(message: &ServerMessage<'_>) -> Message {
    Message::Text(serde_json::to_string(message).unwrap_or_else(|_| "{}".to_string()))
}
