use super::{QueueInsertAt, QueuedSubmission, ReplRuntime};
use crate::agent::{InterMessage, InterMessageSource};
use anyhow::Result;
use std::collections::VecDeque;
use std::sync::{Arc, Mutex, MutexGuard};

/// 把 TUI 用户队列中的「请求间隔」项提供给当前模型回合。
pub(super) struct ReplQueueSource {
    queue: Arc<Mutex<VecDeque<QueuedSubmission>>>,
}

impl ReplQueueSource {
    fn lock(&self) -> MutexGuard<'_, VecDeque<QueuedSubmission>> {
        self.queue.lock().unwrap_or_else(|error| error.into_inner())
    }

    /// 查看下一条请求间隔消息，不改变队列。
    fn peek_request(&self) -> Option<InterMessage> {
        let queue = self.lock();
        queue.iter().find_map(|item| {
            if item.insert_at != QueueInsertAt::Request {
                return None;
            }
            let chat = item.clipboard.to_chat_input(&item.text);
            if chat.message.trim().is_empty() && chat.image_url.is_none() {
                return None;
            }
            Some(InterMessage::queued_user(
                item.id.clone(),
                chat.message,
                chat.image_url.into_iter().collect(),
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

    /// 返回用户提交队列的共享句柄。
    ///
    /// 持有者侧的跨进程受理任务要往队列里放跟随端上行的一轮，而它拿不到
    /// `ReplRuntime` 的借用（主循环正卡在读键上），只能持有这个 `Arc`。
    ///
    /// 返回:
    /// - 队列句柄
    pub(in crate::cli) fn submission_queue_handle(
        &self,
    ) -> Arc<Mutex<VecDeque<QueuedSubmission>>> {
        Arc::clone(&self.submission_queue)
    }

    /// 按当前队列长度夹紧管理面板高亮。
    pub(super) fn clamp_queue_panel(&mut self) {
        let len = self.queue_len();
        self.queue_panel.clamp(len);
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
        })
    }

    /// 取出全部轮次间隔提交，留下请求间隔项给下一次模型请求。
    pub(in crate::cli) fn take_turn_interval_queue(&mut self) -> Vec<QueuedSubmission> {
        let taken = {
            let mut queue = self.lock_queue();
            let mut taken = Vec::new();
            let mut kept = VecDeque::new();
            while let Some(item) = queue.pop_front() {
                match item.insert_at {
                    QueueInsertAt::Turn => taken.push(item),
                    QueueInsertAt::Request => kept.push_back(item),
                }
            }
            *queue = kept;
            taken
        };
        self.clamp_queue_panel();
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
