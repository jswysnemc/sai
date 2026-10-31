use super::{QueueInsertAt, QueuedSubmission, ReplRuntime};
use crate::agent::{InterMessage, InterMessageSource};
use anyhow::Result;
use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};

/// 把 TUI 用户队列中的「请求间隔」项提供给当前模型回合。
pub(super) struct ReplQueueSource {
    queue: Arc<Mutex<VecDeque<QueuedSubmission>>>,
    paused: Arc<AtomicBool>,
}

impl ReplQueueSource {
    fn lock(&self) -> MutexGuard<'_, VecDeque<QueuedSubmission>> {
        self.queue.lock().unwrap_or_else(|error| error.into_inner())
    }

    /// 查看下一条请求间隔消息，不改变队列。
    fn peek_request(&self) -> Option<InterMessage> {
        if self.paused.load(Ordering::Acquire) {
            return None;
        }
        let queue = self.lock();
        queue.iter().find_map(|item| {
            if item.insert_at != QueueInsertAt::Request {
                return None;
            }
            let chat = item.clipboard.to_chat_input(&item.text);
            if chat.message.trim().is_empty() && chat.image_urls.is_empty() {
                return None;
            }
            Some(InterMessage::queued_user(
                item.id.clone(),
                chat.message,
                chat.image_urls,
            ))
        })
    }

    /// 在 provider 成功接收后按标识移除排队项。
    fn acknowledge_id(&self, message_id: &str) {
        let mut queue = self.lock();
        if let Some(index) = queue.iter().position(|item| item.id == message_id) {
            queue.remove(index);
        }
    }
}

#[async_trait::async_trait]
impl InterMessageSource for ReplQueueSource {
    /// 查看下一条请求间隔用户消息。
    ///
    /// 轮次间隔项即使排在前面也不阻挡请求间隔投递。
    async fn peek(&self) -> Result<Option<InterMessage>> {
        Ok(self.peek_request())
    }

    /// 确认请求间隔消息已进入成功的模型请求。
    async fn acknowledge(&self, message_id: &str) -> Result<()> {
        self.acknowledge_id(message_id);
        Ok(())
    }
}

impl ReplRuntime {
    /// 加锁访问用户提交队列。
    pub(super) fn lock_queue(&self) -> MutexGuard<'_, VecDeque<QueuedSubmission>> {
        self.submission_queue
            .lock()
            .unwrap_or_else(|error| error.into_inner())
    }

    /// 返回当前用户队列长度。
    pub(in crate::cli) fn queue_len(&self) -> usize {
        self.lock_queue().len()
    }

    /// 按当前队列长度夹紧管理面板高亮。
    pub(super) fn clamp_queue_panel(&mut self) {
        let ids = self
            .lock_queue()
            .iter()
            .map(|item| item.id.clone())
            .collect::<Vec<_>>();
        self.queue_panel.sync_ids(&ids);
    }

    /// 返回当前排队项的快照。
    pub(super) fn queued_items(&self) -> Vec<QueuedSubmission> {
        self.lock_queue().iter().cloned().collect()
    }

    /// 把当前用户队列接到活动模型回合的请求间隙。
    ///
    /// 网格入站回执走同一条请求间隙（`next_gap_message`），不经轮次排空。
    pub(in crate::cli) fn inter_message_source(&self) -> Arc<dyn InterMessageSource> {
        Arc::new(ReplQueueSource {
            queue: Arc::clone(&self.submission_queue),
            paused: Arc::clone(&self.submission_queue_paused),
        })
    }

    /// 【终端队列】【暂停】暂停自动投递，保留全部条目供用户查看和编辑。
    ///
    /// 参数: 无
    /// 返回: 无
    pub(in crate::cli) fn pause_submission_queue(&mut self) {
        self.submission_queue_paused.store(true, Ordering::Release);
    }

    /// 【终端队列】【恢复】用户主动继续后允许后续请求或轮次消费队列。
    ///
    /// 参数: 无
    /// 返回: 无
    pub(in crate::cli) fn resume_submission_queue(&mut self) {
        self.submission_queue_paused.store(false, Ordering::Release);
    }

    /// 【终端队列】【主动继续】只允许用户输入解除暂停，自动续轮和后台回执不能恢复队列。
    ///
    /// 参数: `submission` 为即将执行的轮次输入
    /// 返回: 无
    pub(in crate::cli) fn resume_queue_for_submission(
        &mut self,
        submission: &crate::runner::RunnerSubmission,
    ) {
        if matches!(
            &submission.kind,
            crate::runner::RunnerSubmissionKind::UserInput(input) if input.automatic_input.is_none()
        ) {
            self.resume_submission_queue();
        }
    }

    /// 【终端队列】【空闲调度】判断是否存在未暂停的轮次消息，避免空闲主循环重复消费失败队列。
    ///
    /// 参数: 无
    /// 返回: 可以自动开始下一轮时为真；仅有请求间隔消息时为假
    pub(in crate::cli) fn has_ready_turn_submission(&self) -> bool {
        !self.submission_queue_paused.load(Ordering::Acquire)
            && self
                .lock_queue()
                .iter()
                .any(|item| item.insert_at == QueueInsertAt::Turn)
    }

    /// 【终端队列】【逐条执行】只取出下一条轮次间隔消息，其余消息继续留在管理面板中。
    ///
    /// 参数: 无
    /// 返回: 原队列位置与待执行消息；没有轮次间隔消息时为空
    pub(in crate::cli) fn take_next_turn_submission(
        &mut self,
    ) -> Option<(usize, QueuedSubmission)> {
        if self.submission_queue_paused.load(Ordering::Acquire) {
            return None;
        }
        let taken = {
            let mut queue = self.lock_queue();
            let index = queue
                .iter()
                .position(|item| item.insert_at == QueueInsertAt::Turn)?;
            queue.remove(index).map(|item| (index, item))
        };
        self.clamp_queue_panel();
        taken
    }

    /// 【终端队列】【启动恢复】恢复尚未启动的提交，不改变附件、模式和插入点。
    ///
    /// 参数:
    /// - `index`: 取出消息时的队列位置
    /// - `item`: 尚未执行的完整提交
    ///
    /// 返回: 无
    pub(in crate::cli) fn restore_queued_submission(
        &mut self,
        index: usize,
        item: QueuedSubmission,
    ) {
        {
            let mut queue = self.lock_queue();
            let index = index.min(queue.len());
            queue.insert(index, item);
        }
        self.clamp_queue_panel();
    }

    /// 【终端队列】【测试取出】重复调用正式逐条入口，返回全部轮次间隔提交。
    ///
    /// 参数: 无
    /// 返回: 供测试检查的轮次消息列表，请求间隔消息保持不动
    #[cfg(test)]
    pub(in crate::cli) fn take_turn_interval_queue(&mut self) -> Vec<QueuedSubmission> {
        let mut taken = Vec::new();
        while let Some((_, item)) = self.take_next_turn_submission() {
            taken.push(item);
        }
        taken
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::AgentMode;
    use crate::cli::repl_clipboard::ReplClipboardState;

    fn queued(text: &str, insert_at: QueueInsertAt) -> QueuedSubmission {
        let mut item = QueuedSubmission::new(
            AgentMode::Yolo,
            text.to_string(),
            ReplClipboardState::default(),
        );
        item.insert_at = insert_at;
        item
    }

    fn source_with(items: Vec<QueuedSubmission>) -> ReplQueueSource {
        ReplQueueSource {
            queue: Arc::new(Mutex::new(items.into())),
            paused: Arc::new(AtomicBool::new(false)),
        }
    }

    #[test]
    fn peek_skips_turn_interval_items() {
        let turn = queued("later", QueueInsertAt::Turn);
        let request = queued("now", QueueInsertAt::Request);
        let source = source_with(vec![turn, request.clone()]);

        let peeked = source.peek_request().expect("request item should peek");
        assert_eq!(peeked.id, request.id);
        assert_eq!(peeked.prompt, "now");
        assert_eq!(source.lock().len(), 2);
    }

    #[test]
    fn acknowledge_removes_only_the_matched_id() {
        let first = queued("one", QueueInsertAt::Request);
        let second = queued("two", QueueInsertAt::Request);
        let source = source_with(vec![first.clone(), second.clone()]);

        source.acknowledge_id(&first.id);
        let remaining: Vec<String> = source.lock().iter().map(|item| item.text.clone()).collect();
        assert_eq!(remaining, vec!["two".to_string()]);
    }
}
