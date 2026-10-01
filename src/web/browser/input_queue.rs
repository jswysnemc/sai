//! 合并尚未执行的连续输入，限制积压并保留按键与点击边界。

use super::protocol::ClientMessage;
use std::collections::VecDeque;
use std::sync::Mutex;
use tokio::sync::Notify;

const MAX_PENDING_INPUTS: usize = 256;

#[derive(Default)]
pub(super) struct InputQueue {
    messages: Mutex<VecDeque<ClientMessage>>,
    ready: Notify,
}

impl InputQueue {
    /// 【浏览器面板】【输入入队】合并队尾兼容事件；关键事件之间不跨越合并。
    /// @param message 为新输入
    /// @returns 队列达到上限时返回错误，由连接服务停止接收
    pub(super) fn push(&self, message: ClientMessage) -> Result<(), &'static str> {
        let mut messages = self.messages.lock().unwrap();
        if let Some(previous) = messages.back_mut() {
            if merge(previous, &message) {
                return Ok(());
            }
        }
        if messages.len() >= MAX_PENDING_INPUTS {
            return Err("browser input queue is full");
        }
        messages.push_back(message);
        self.ready.notify_one();
        Ok(())
    }

    /// 【浏览器面板】【输入出队】等待并取出最早的待处理事件。
    /// @returns 一条保序输入；连接结束时由调用方取消等待
    pub(super) async fn next(&self) -> ClientMessage {
        loop {
            let notified = self.ready.notified();
            if let Some(message) = self.messages.lock().unwrap().pop_front() {
                return message;
            }
            notified.await;
        }
    }
}

/// 【浏览器面板】【输入合并】只合并相邻且语义兼容的移动、滚轮与尺寸消息。
/// @param previous 为队尾；next 为新输入
/// @returns 已合并时为 true；点击、按键和修饰键变化构成顺序边界
fn merge(previous: &mut ClientMessage, next: &ClientMessage) -> bool {
    match (previous, next) {
        (ClientMessage::Mouse(previous), ClientMessage::Mouse(next))
            if previous.kind == next.kind
                && previous.modifiers == next.modifiers
                && previous.button == next.button =>
        {
            match next.kind.as_str() {
                "move" => {
                    *previous = next.clone();
                    true
                }
                // 滚轮只在同一个指针位置合并，避免改变嵌套滚动区域的事件目标
                "wheel" if previous.x == next.x && previous.y == next.y => {
                    previous.delta_x += next.delta_x;
                    previous.delta_y += next.delta_y;
                    true
                }
                _ => false,
            }
        }
        (
            ClientMessage::Resize {
                width,
                height,
                scale,
            },
            ClientMessage::Resize {
                width: next_width,
                height: next_height,
                scale: next_scale,
            },
        ) => {
            *width = *next_width;
            *height = *next_height;
            *scale = *next_scale;
            true
        }
        _ => false,
    }
}
