//! 跟随模式（P4）：把会话持有者下行的事件渲染进 TUI transcript。
//!
//! 本进程是观察者时，会话由另一个 sai 实例驱动：本终端不再执行轮次，
//! 只把对端的一轮输出按纯文本补进 transcript，让用户知道那边在做什么。
//!
//! 刻意不把它做成完整的远端渲染（`record_runner_event` 那套流式渲染）：
//! 那条路径与轮次生命周期、权限弹窗、工作区状态深度耦合，而观察者侧拿到的
//! 是已经组装好的 `WebEvent`，反推回 `RunnerEvent` 会引入一整类难以验证的
//! 状态错位。**先看得到、且不会坏**，比渲染得漂亮更重要。

use super::ReplRuntime;
use crate::i18n::text as t;
use crate::web::runs::WebEvent;
use anyhow::Result;
use tokio::sync::mpsc;

/// 跟随模式下累积的远端输出。
///
/// 正文分片逐条到达，攒到轮次结束再整段落进 transcript：分片插进去会让
/// transcript 每行一个 meta 单元，滚屏时无法阅读。
#[derive(Default)]
pub(in crate::cli) struct FollowBuffer {
    text: String,
}

impl ReplRuntime {
    /// 接入或替换跟随流。
    ///
    /// 参数:
    /// - `events`: 持有者下行事件的接收端
    ///
    /// 返回:
    /// - 无
    pub(in crate::cli) fn follow_remote_stream(
        &mut self,
        events: mpsc::UnboundedReceiver<WebEvent>,
    ) {
        self.follow_events = Some(events);
        self.follow_buffer = FollowBuffer::default();
    }

    /// 结束跟随模式（本进程已接管会话）。
    ///
    /// 返回:
    /// - 无
    pub(in crate::cli) fn stop_following(&mut self) {
        self.follow_events = None;
        self.follow_buffer = FollowBuffer::default();
    }

    /// 渲染当前积压的远端事件。
    ///
    /// 只做非阻塞排空：跟随模式下主循环仍卡在读键上，这里由空闲重绘节拍
    /// （`process_idle_tick`）驱动，不需要也不该阻塞输入。
    ///
    /// 返回:
    /// - 是否写入了 transcript（调用方据此决定是否重绘）
    pub(in crate::cli) fn drain_follow_events(&mut self) -> Result<bool> {
        let Some(events) = self.follow_events.as_mut() else {
            return Ok(false);
        };
        let mut pending = Vec::new();
        while let Ok(event) = events.try_recv() {
            pending.push(event);
        }
        if pending.is_empty() {
            return Ok(false);
        }
        for event in pending {
            self.render_follow_event(event)?;
        }
        Ok(true)
    }

    /// 渲染一条远端事件。
    ///
    /// 参数:
    /// - `event`: 持有者下行的事件
    ///
    /// 返回:
    /// - 无
    fn render_follow_event(&mut self, event: WebEvent) -> Result<()> {
        match event.kind.as_str() {
            // 新一轮开始：先把上一轮没收尾的输出落下去
            "run.started" => self.flush_follow_buffer()?,
            "message.content.delta" => {
                if let Some(text) = event.payload.get("text").and_then(|text| text.as_str()) {
                    self.follow_buffer.text.push_str(text);
                }
            }
            "tool.call.started" => {
                self.flush_follow_buffer()?;
                let name = event
                    .payload
                    .get("name")
                    .and_then(|name| name.as_str())
                    .unwrap_or("tool");
                self.record_meta(format!("· {name}"))?;
            }
            "permission.requested" => {
                self.flush_follow_buffer()?;
                self.record_meta(
                    t(
                        "the holder is asking for permission in its own terminal",
                        "持有者正在它自己的终端里请求权限",
                    )
                    .to_string(),
                )?;
            }
            "question.requested" => {
                self.flush_follow_buffer()?;
                self.record_meta(
                    t(
                        "the holder is asking a question in its own terminal",
                        "持有者正在它自己的终端里提问",
                    )
                    .to_string(),
                )?;
            }
            "run.completed" => self.flush_follow_buffer()?,
            "run.interrupted" => {
                self.flush_follow_buffer()?;
                self.record_meta(
                    t("the holder interrupted this run", "持有者中断了这一轮").to_string(),
                )?;
            }
            "run.failed" => {
                self.flush_follow_buffer()?;
                let message = event
                    .payload
                    .get("message")
                    .and_then(|message| message.as_str())
                    .unwrap_or("");
                self.record_failure(format!(
                    "{}：{message}",
                    t("the holder's run failed", "持有者这一轮失败")
                ))?;
            }
            _ => {}
        }
        Ok(())
    }

    /// 把累积的远端正文整段落进 transcript。
    ///
    /// 返回:
    /// - 无
    fn flush_follow_buffer(&mut self) -> Result<()> {
        let text = std::mem::take(&mut self.follow_buffer.text);
        let text = text.trim();
        if text.is_empty() {
            return Ok(());
        }
        self.record_meta(text.to_string())
    }
}
