use super::subagent_state::SubagentSnapshot;
use super::subagent_timeline::SubagentTimelineEntry;
use chrono::Utc;
use serde::Serialize;
use std::collections::VecDeque;
use tokio::sync::broadcast;

const EVENT_CAPACITY: usize = 1024;

/// 子智能体详情流事件。
#[derive(Clone, Debug, Serialize)]
pub(crate) struct SubagentStreamEvent {
    pub(crate) sequence: u64,
    pub(crate) timestamp: String,
    pub(crate) snapshot: SubagentSnapshot,
    pub(crate) timeline: Vec<SubagentTimelineEntry>,
}

/// 保存子智能体详情快照并实时广播变化。
pub(crate) struct SubagentEventJournal {
    next_sequence: u64,
    events: VecDeque<SubagentStreamEvent>,
    sender: broadcast::Sender<SubagentStreamEvent>,
}

impl SubagentEventJournal {
    /// 创建空事件日志。
    ///
    /// 返回:
    /// - 子智能体事件日志
    pub(crate) fn new() -> Self {
        let (sender, _) = broadcast::channel(EVENT_CAPACITY);
        Self {
            next_sequence: 1,
            events: VecDeque::new(),
            sender,
        }
    }

    /// 发布最新详情快照。
    ///
    /// 参数:
    /// - `snapshot`: 子智能体状态快照
    /// - `timeline`: 子智能体执行时间线
    pub(crate) fn publish(
        &mut self,
        snapshot: SubagentSnapshot,
        timeline: Vec<SubagentTimelineEntry>,
    ) {
        let event = SubagentStreamEvent {
            sequence: self.next_sequence,
            timestamp: Utc::now().to_rfc3339(),
            snapshot,
            timeline,
        };
        self.next_sequence = self.next_sequence.saturating_add(1);
        self.events.push_back(event.clone());
        while self.events.len() > EVENT_CAPACITY {
            self.events.pop_front();
        }
        let _ = self.sender.send(event);
    }

    /// 返回指定序号之后的事件。
    ///
    /// 参数:
    /// - `after`: 已接收的最后事件序号
    ///
    /// 返回:
    /// - 待补发事件
    pub(crate) fn events_after(&self, after: u64) -> Vec<SubagentStreamEvent> {
        self.events
            .iter()
            .filter(|event| event.sequence > after)
            .cloned()
            .collect()
    }

    /// 订阅后续实时事件。
    ///
    /// 返回:
    /// - 广播接收器
    pub(crate) fn subscribe(&self) -> broadcast::Receiver<SubagentStreamEvent> {
        self.sender.subscribe()
    }
}
