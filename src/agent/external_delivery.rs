use super::{ExternalEventBatch, ExternalEventMonitor};
use crate::tools::command::{
    acknowledge_background_attention, acknowledge_background_completions,
    poll_background_attention, BackgroundCommandStore,
};
use crate::tools::mesh::{acknowledge_mesh_messages, next_pending};
use crate::tools::subagent_state::{acknowledge_finished_notices, pending_finished_notices};
use anyhow::Result;

impl ExternalEventMonitor {
    /// 【自动续聊】【通知校验】检查排队快照是否仍属于当前会话的待处理事件。
    ///
    /// 参数:
    /// - `batch`: 监听器已排队的完成通知
    ///
    /// 返回:
    /// - 通知尚未确认、读取或清理时返回 true
    pub(crate) fn is_pending(&self, batch: &ExternalEventBatch) -> Result<bool> {
        let goal_id = self
            .state
            .goal()?
            .filter(|goal| goal.status.accepts_external_wake())
            .map(|goal| goal.id);
        if let Some(notice) = &batch.background_attention {
            return Ok(poll_background_attention(
                &self.paths,
                self.state.session_id(),
                goal_id.as_deref(),
            )?
            .iter()
            .any(|pending| pending.event_id == notice.event_id));
        }
        if !batch.background_task_ids.is_empty() {
            let tasks = BackgroundCommandStore::new(self.paths.state_dir.clone()).load()?;
            return Ok(tasks.iter().any(|task| {
                batch.background_task_ids.contains(&task.id)
                    && task.owned_by_session(self.state.session_id())
                    && task.goal_id == goal_id
                    && task.status != "running"
                    && !task.completion_notified
            }));
        }
        if !batch.subagent_ids.is_empty() {
            let owner_key = self.state.state_dir().display().to_string();
            return Ok(pending_finished_notices(&owner_key).iter().any(|notice| {
                notice.goal_id == goal_id && batch.subagent_ids.contains(&notice.id)
            }));
        }
        if !batch.mesh_message_ids.is_empty() {
            return Ok(
                next_pending(self.state.state_dir(), self.state.session_id())
                    .is_some_and(|envelope| batch.mesh_message_ids.contains(&envelope.id)),
            );
        }
        Ok(false)
    }

    /// 【自动续聊】【通知确认】确认已进入成功请求的事件，不删除后台日志。
    ///
    /// 参数:
    /// - `batch`: 已成功交付的完成通知
    ///
    /// 返回:
    /// - 各来源确认结果；失败时保留可重试状态
    pub(crate) fn acknowledge(&self, batch: &ExternalEventBatch) -> Result<()> {
        let owner_key = self.state.state_dir().display().to_string();
        acknowledge_background_completions(
            &self.paths,
            self.state.session_id(),
            &batch.background_task_ids,
        )?;
        if let Some(notice) = &batch.background_attention {
            acknowledge_background_attention(
                &self.paths,
                self.state.session_id(),
                std::slice::from_ref(&notice.task_id),
            )?;
        }
        acknowledge_finished_notices(&owner_key, &batch.subagent_ids);
        acknowledge_mesh_messages(self.state.state_dir(), &batch.mesh_message_ids);
        Ok(())
    }
}
