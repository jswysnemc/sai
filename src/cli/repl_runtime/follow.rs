//! 跟随模式（P4）：把会话持有者下行的事件渲染进 TUI transcript。
//!
//! 本进程是观察者时，会话由另一个 sai 实例驱动：本终端不再执行轮次，
//! 只把对端的输出渲染进 transcript。正文分片经 `push_chunk` 进入 live
//! tail 实时滚动——与持有者本地的流式渲染共用同一条路径，但没有轮次
//! 生命周期、权限弹窗的耦合：交互类事件只落一行提示，权限仍在持有者
//! 的终端里由它自己处理。

use super::ReplRuntime;
use crate::i18n::text as t;
use crate::llm::{ChatStreamChunk, ChatStreamKind};
use crate::web::runs::WebEvent;
use anyhow::Result;
use tokio::sync::mpsc;

/// 跟随模式下累积的远端输出。
///
/// 工具调用等非正文事件的提示文本；正文分片已经实时进入 live tail，
/// 不再需要整轮缓存。
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
            // 持有者广播的用户回显：本终端（或其它跟随端）发出的消息也要看得见，
            // 否则跟随端只见回答不见提问
            super::super::repl_session_link::USER_SUBMITTED_EVENT => {
                self.flush_follow_buffer()?;
                self.transcript.finalize_live_tail();
                let text = event
                    .payload
                    .get("input")
                    .and_then(|input| input.as_str())
                    .unwrap_or_default()
                    .to_string();
                if !text.trim().is_empty() {
                    self.record_user(crate::agent::AgentMode::Yolo, text, false)?;
                }
            }
            // 新一轮开始：收尾上一轮，进入思考动效——与持有者同款状态行
            "run.started" => {
                self.flush_follow_buffer()?;
                self.transcript.finalize_live_tail();
                self.transcript.set_work_status(
                    crate::render::work_status::WorkStatus::WaitingResponse,
                );
                self.arm_live_ticker();
                self.sync_transcript(true)?;
            }
            // 正文与思考分片直接进入 live tail，实时滚动
            "message.content.delta" => {
                if let Some(text) = event.payload.get("text").and_then(|text| text.as_str()) {
                    if !text.is_empty() {
                        self.transcript.push_chunk(&ChatStreamChunk {
                            kind: ChatStreamKind::Content,
                            text: text.to_string(),
                        });
                        self.throttled_live_sync()?;
                    }
                }
            }
            "message.reasoning.delta" => {
                if let Some(text) = event.payload.get("text").and_then(|text| text.as_str()) {
                    if !text.is_empty() {
                        self.transcript.push_chunk(&ChatStreamChunk {
                            kind: ChatStreamKind::Reasoning,
                            text: text.to_string(),
                        });
                        self.throttled_live_sync()?;
                    }
                }
            }
            "tool.call.started" => {
                self.flush_follow_buffer()?;
                self.transcript.finalize_live_tail();
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
            "run.completed" => {
                self.flush_follow_buffer()?;
                self.transcript.finalize_live_tail();
                self.transcript.clear_work_status();
                self.sync_transcript(false)?;
            }
            "run.interrupted" => {
                self.flush_follow_buffer()?;
                self.transcript.finalize_live_tail();
                self.transcript.clear_work_status();
                self.sync_transcript(false)?;
                self.record_meta(
                    t("the holder interrupted this run", "持有者中断了这一轮").to_string(),
                )?;
            }
            "run.failed" => {
                self.flush_follow_buffer()?;
                self.transcript.finalize_live_tail();
                self.transcript.clear_work_status();
                self.sync_transcript(false)?;
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

    /// 把累积的远端提示文本整段落进 transcript。
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
