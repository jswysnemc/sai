//! 面板控制命令的执行与导航任务管理。

use super::frame_delivery::text;
use super::input_queue::InputQueue;
use super::protocol::{ClientMessage, ServerMessage};
use crate::browser::{normalize_url, BrowserSession};
use anyhow::Result;
use axum::extract::ws::Message;
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio::task::JoinSet;

/// 【浏览器面板】【输入执行】独立消费合并队列，关键输入按顺序等待完成。
/// @param session 为共享会话；queue 为待处理输入；outgoing 为文本输出通道
/// @returns 连接结束时由调用方取消，派生导航任务随之释放
pub(super) async fn run(
    session: Arc<BrowserSession>,
    queue: Arc<InputQueue>,
    outgoing: mpsc::UnboundedSender<Message>,
) {
    let mut navigation = JoinSet::new();
    loop {
        tokio::select! {
            message = queue.next() => {
                if message.is_slow() {
                    let session = session.clone();
                    let outgoing = outgoing.clone();
                    navigation.spawn(async move {
                        report(execute(&session, message).await, &outgoing);
                    });
                } else {
                    report(execute(&session, message).await, &outgoing);
                }
            }
            _ = navigation.join_next(), if !navigation.is_empty() => {}
        }
    }
}

/// 【浏览器面板】【执行结果】将操作失败原因发给面板。
/// @param result 为操作结果；outgoing 为文本输出通道
/// @returns 无
fn report(result: Result<()>, outgoing: &mpsc::UnboundedSender<Message>) {
    if let Err(error) = result {
        let message = format!("{error:#}");
        let _ = outgoing.send(text(&ServerMessage::Error { message: &message }));
    }
}

/// 【浏览器面板】【操作执行】把面板消息映射到浏览器会话操作。
/// @param session 为浏览器会话；message 为面板消息
/// @returns 操作结果
pub(super) async fn execute(session: &BrowserSession, message: ClientMessage) -> Result<()> {
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
