use super::{RunKind, RunManager};
use crate::agent::{InterMessage, InterMessageSource};
use crate::web::runs::checkpoint::RunCheckpointStatus;
use crate::web::runs::WebEvent;
use anyhow::{bail, Result};
use serde_json::json;

/// 把同一 Web 会话的排队输入提供给活动模型回合。
pub(super) struct WebMessageQueue {
    manager: RunManager,
    session_key: String,
    target_run_id: String,
}

impl WebMessageQueue {
    /// 创建活动运行对应的消息来源。
    ///
    /// 参数:
    /// - `manager`: Web 运行管理器
    /// - `session_key`: 工作区与会话组成的调度键
    /// - `target_run_id`: 接收排队消息的活动运行标识
    ///
    /// 返回:
    /// - 可在模型消息间隙读取队列的来源
    pub(super) fn new(manager: RunManager, session_key: String, target_run_id: String) -> Self {
        Self {
            manager,
            session_key,
            target_run_id,
        }
    }

    /// 【Web队列】【策略筛选】参数 active_mode 为消费时的实时策略；返回匹配项，其他任务留待独立执行。
    async fn peek_matching(
        &self,
        active_mode: Option<crate::agent::AgentMode>,
    ) -> Result<Option<InterMessage>> {
        let queues = self.manager.queued.lock().await;
        let Some(queue) = queues.get(&self.session_key) else {
            return Ok(None);
        };
        for queued in queue {
            // 控制命令保持独立运行，不能越过前面的压缩/续轮去合并用户消息
            if queued.request.kind == RunKind::GoalContinuation {
                continue;
            }
            if queued.request.kind != RunKind::Conversation {
                return Ok(None);
            }
            if queued.request.insert_at != super::QueueInsertAt::Request {
                continue;
            }
            // 1. 【Web队列】【模式保留】不同策略的任务保持独立，不能作为普通指导丢弃模式
            if active_mode.is_some_and(|active| {
                crate::agent::AgentMode::parse(queued.request.mode.as_deref()).ok() != Some(active)
            }) {
                continue;
            }
            return Ok(Some(InterMessage::queued_user(
                queued.info.run_id.clone(),
                queued.info.input.clone(),
                queued.info.image_urls.clone(),
            )));
        }
        Ok(None)
    }
}

#[async_trait::async_trait]
impl InterMessageSource for WebMessageQueue {
    /// 【Web队列】【读取】无参数；返回下一条排队消息，不确认消费。
    async fn peek(&self) -> Result<Option<InterMessage>> {
        self.peek_matching(None).await
    }

    /// 【Web队列】【实时模式】参数 mode 为 Agent 当前模式；返回可合并消息，避免使用启动时的旧策略。
    async fn peek_for_mode(&self, mode: crate::agent::AgentMode) -> Result<Option<InterMessage>> {
        self.peek_matching(Some(mode)).await
    }

    /// 在 provider 成功接收消息后确认队首输入。
    async fn acknowledge(&self, message_id: &str) -> Result<()> {
        let _scheduling = self.manager.scheduling.lock().await;
        let queued = {
            let mut queues = self.manager.queued.lock().await;
            let Some(queue) = queues.get_mut(&self.session_key) else {
                bail!("queued message no longer exists: {message_id}");
            };
            let Some(position) = queue
                .iter()
                .position(|queued| queued.info.run_id == message_id)
            else {
                bail!("queued message no longer exists: {message_id}");
            };

            // 1. 先持久化终态，失败时保持内存队列不变
            self.manager
                .checkpoints
                .update_status(message_id, RunCheckpointStatus::Completed)?;
            // 2. 再按 id 移除，避免 launch_next 把同一消息启动成独立回合
            let queued = queue
                .remove(position)
                .expect("validated queued message must still exist");
            let remove_queue = queue.is_empty();
            if remove_queue {
                queues.remove(&self.session_key);
            }
            queued
        };

        let bus = self
            .manager
            .session_bus(&queued.info.workspace_id, &queued.info.session_id)
            .await;
        let _ = bus.emit(WebEvent::new(
            &queued.info.run_id,
            &queued.info.workspace_id,
            &queued.info.session_id,
            "run.merged",
            json!({ "target_run_id": self.target_run_id }),
        ));
        Ok(())
    }
}

#[cfg(test)]
#[path = "message_queue_mode_tests.rs"]
mod mode_tests;
