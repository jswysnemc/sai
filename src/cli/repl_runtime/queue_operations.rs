//! 队列内容操作，附件和稳定标识随消息整体移动。

use super::{QueuedSubmission, ReplRuntime};
use std::collections::VecDeque;

/// 【用户队列】【标识校验】同一锁内查询当前选中消息，后台消费后不操作替补消息。
/// 参数: queue 为加锁队列，id 为选中标识；返回: 消息当前下标。
fn selected_index(queue: &VecDeque<QueuedSubmission>, id: Option<&str>) -> Option<usize> {
    let id = id?;
    queue.iter().position(|item| item.id == id)
}

impl ReplRuntime {
    /// 【用户队列】【时机切换】参数: 无；返回: 无，当前消息不存在时忽略。
    pub(super) fn toggle_queued_insert_at(&mut self) {
        let mut queue = self.lock_queue();
        if let Some(index) = selected_index(&queue, self.queue_panel.selected_id.as_deref()) {
            queue[index].insert_at = queue[index].insert_at.toggle();
        }
    }

    /// 【用户队列】【删除记录】移出选中消息并记录原下标，最多保留二十次删除以便恢复。
    /// 参数: 无；返回: 无，附件与原消息共同进入恢复栈。
    pub(super) fn delete_queued_at(&mut self) {
        let item = {
            let mut queue = self.lock_queue();
            selected_index(&queue, self.queue_panel.selected_id.as_deref())
                .and_then(|index| queue.remove(index).map(|item| (index, item)))
        };
        if let Some((index, item)) = item {
            self.queue_panel.deleted.push_back((index, item));
            if self.queue_panel.deleted.len() > 20 {
                self.queue_panel.deleted.pop_front();
            }
            self.queue_panel.select(index);
        }
        self.clamp_queue_panel();
    }

    /// 【用户队列】【恢复删除】只恢复显式删除记录，不恢复已发送或已取回编辑的消息。
    /// 参数: 无；返回: 是否恢复，消息重新使用原稳定标识和附件。
    pub(super) fn restore_deleted_queued(&mut self) -> bool {
        let Some((index, item)) = self.queue_panel.deleted.pop_back() else {
            return false;
        };
        let id = item.id.clone();
        let mut queue = self.lock_queue();
        if queue.iter().any(|entry| entry.id == id) {
            return false;
        }
        let index = index.min(queue.len());
        queue.insert(index, item);
        drop(queue);
        self.queue_panel.select(index);
        self.queue_panel.selected_id = Some(id);
        self.clamp_queue_panel();
        true
    }

    /// 【用户队列】【调序置顶】同一消费时机内按列表顺序投递，置顶不改变消息时机。
    /// 参数: delta 为相邻移动量，top 为是否直接移到首项；返回: 无。
    pub(super) fn move_queued(&mut self, delta: isize, top: bool) {
        let moved = {
            let mut queue = self.lock_queue();
            selected_index(&queue, self.queue_panel.selected_id.as_deref()).map(|index| {
                let target = if top {
                    0
                } else {
                    index.saturating_add_signed(delta).min(queue.len() - 1)
                };
                if let Some(item) = queue.remove(index) {
                    queue.insert(target, item);
                }
                target
            })
        };
        if let Some(index) = moved {
            self.queue_panel.selected = index;
        }
        self.clamp_queue_panel();
    }

    /// 【用户队列】【取回编辑】移出当前消息，取回不进入删除恢复栈，避免重复提交。
    /// 参数: 无；返回: 完整消息，当前选中项消失时为空。
    pub(super) fn take_queued_at(&mut self) -> Option<QueuedSubmission> {
        let item = {
            let mut queue = self.lock_queue();
            selected_index(&queue, self.queue_panel.selected_id.as_deref())
                .and_then(|index| queue.remove(index))
        };
        self.clamp_queue_panel();
        item
    }

    /// 【用户队列】【草稿恢复】保留原时机、权限模式和附件，不用删除历史记录模拟编辑。
    /// 参数: item 为排队消息；返回: 无。
    pub(super) fn apply_queued_to_stream_draft(&mut self, item: QueuedSubmission) {
        self.stream_draft.clipboard = item.clipboard;
        self.stream_draft.mode = Some(item.mode);
        self.stream_draft.text = item.text;
        self.stream_draft.cursor = self.stream_draft.text.chars().count();
        self.stream_draft.slash_selection = 0;
        self.stream_draft.is_pasted = false;
        self.queue_draft_timing = item.insert_at;
    }
}
