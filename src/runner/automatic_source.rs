use super::{AutomaticInput, AutomaticInputKind};
use crate::agent::{
    Agent, ExternalEventBatch, ExternalEventMonitor, InterMessage, InterMessageKind,
    InterMessageSource,
};
use anyhow::Result;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/// 【自动续聊】【消息投递】将自动输入接入统一的模型消息间隙和确认流程。
pub(super) struct AutomaticInputSource {
    message: InterMessage,
    pending: AtomicBool,
    batch: Option<ExternalEventBatch>,
    monitor: ExternalEventMonitor,
    next: Option<Arc<dyn InterMessageSource>>,
}

impl AutomaticInputSource {
    /// 【自动续聊】【消息投递】解析自动输入并丢弃已消费的排队通知。
    ///
    /// 参数:
    /// - `input`: Goal 续作或外部完成输入
    /// - `agent`: 当前会话，用于读取 Goal 和确认来源
    /// - `next`: 当前用户排队消息来源
    ///
    /// 返回:
    /// - 待投递来源；通知失效或 Goal 不再活动时返回空
    pub(super) fn new(
        input: &AutomaticInput,
        agent: &Agent,
        next: Option<Arc<dyn InterMessageSource>>,
    ) -> Result<Option<Self>> {
        let monitor = agent.external_event_monitor();
        if let Some(batch) = &input.external_event {
            if !monitor.is_pending(batch)? {
                return Ok(None);
            }
        }
        let goal = agent.state().goal()?.filter(|goal| goal.status.is_active());
        let Some(prompt) = input.prompt_text(goal.as_ref()) else {
            return Ok(None);
        };
        if prompt.trim().is_empty() {
            return Ok(None);
        }
        let id = input
            .external_event
            .as_ref()
            .map(|batch| batch.event_id().to_string())
            .unwrap_or_else(|| format!("automatic_{}", uuid::Uuid::new_v4().simple()));
        Ok(Some(Self {
            message: InterMessage {
                id,
                kind: match input.kind {
                    AutomaticInputKind::GoalContinuation => InterMessageKind::GoalContinuation,
                    AutomaticInputKind::ExternalCompletion => InterMessageKind::ExternalCompletion,
                },
                prompt,
                display: input.display_text(goal.as_ref()),
                image_urls: Vec::new(),
            },
            pending: AtomicBool::new(true),
            batch: input.external_event.clone(),
            monitor,
            next,
        }))
    }
}

#[async_trait::async_trait]
impl InterMessageSource for AutomaticInputSource {
    /// 【自动续聊】【消息投递】优先读取用户队列，再投递尚未确认的自动输入。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 下一个待投递消息，不提前确认任何通知
    async fn peek(&self) -> Result<Option<InterMessage>> {
        if let Some(next) = &self.next {
            if let Some(message) = next.peek().await? {
                return Ok(Some(message));
            }
        }
        Ok(self
            .pending
            .load(Ordering::Acquire)
            .then(|| self.message.clone()))
    }

    /// 【自动续聊】【通知确认】请求成功后确认自动事件或转交用户队列确认。
    ///
    /// 参数:
    /// - `message_id`: 已进入成功模型请求的消息标识
    ///
    /// 返回:
    /// - 消费确认结果
    async fn acknowledge(&self, message_id: &str) -> Result<()> {
        if message_id == self.message.id {
            if self.pending.load(Ordering::Acquire) {
                if let Some(batch) = &self.batch {
                    self.monitor.acknowledge(batch)?;
                }
                self.pending.store(false, Ordering::Release);
            }
            return Ok(());
        }
        if let Some(next) = &self.next {
            next.acknowledge(message_id).await?;
        }
        Ok(())
    }
}
