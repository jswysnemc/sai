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
}

#[async_trait::async_trait]
impl InterMessageSource for WebMessageQueue {
    /// 查看队首用户消息，不改变队列和检查点。
    async fn peek(&self) -> Result<Option<InterMessage>> {
        let queues = self.manager.queued.lock().await;
        let Some(queued) = queues
            .get(&self.session_key)
            .and_then(|queue| queue.front())
        else {
            return Ok(None);
        };
        // 控制命令保持独立运行，不能越过队首合并后续用户消息
        if queued.request.kind != RunKind::Conversation {
            return Ok(None);
        }
        Ok(Some(InterMessage::queued_user(
            queued.info.run_id.clone(),
            queued.info.input.clone(),
            queued.info.image_urls.clone(),
        )))
    }

    /// 在 provider 成功接收消息后确认队首输入。
    async fn acknowledge(&self, message_id: &str) -> Result<()> {
        let _scheduling = self.manager.scheduling.lock().await;
        let queued = {
            let mut queues = self.manager.queued.lock().await;
            let Some(queue) = queues.get_mut(&self.session_key) else {
                bail!("queued message no longer exists: {message_id}");
            };
            let Some(front) = queue.front() else {
                bail!("queued message no longer exists: {message_id}");
            };
            if front.info.run_id != message_id {
                bail!("queued message order changed before acknowledgement: {message_id}");
            }

            // 1. 先持久化终态，失败时保持内存队列不变
            self.manager
                .checkpoints
                .update_status(message_id, RunCheckpointStatus::Completed)?;
            // 2. 再移除内存队首，避免 launch_next 把同一消息启动成独立回合
            let queued = queue
                .pop_front()
                .expect("validated queued message must still exist");
            let remove_queue = queue.is_empty();
            if remove_queue {
                queues.remove(&self.session_key);
            }
            queued
        };

        let journal = self.manager.journal(message_id).await.unwrap_or_else(|| {
            super::super::EventJournal::persistent(self.manager.checkpoints.event_path(message_id))
        });
        journal.publish(WebEvent::new(
            &queued.info.run_id,
            &queued.info.workspace_id,
            &queued.info.session_id,
            "run.merged",
            json!({ "target_run_id": self.target_run_id }),
        ));
        Ok(())
    }
}
