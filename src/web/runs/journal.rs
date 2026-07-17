use super::WebEvent;
use std::collections::VecDeque;
use std::io::Write;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tokio::sync::broadcast;

const EVENT_JOURNAL_CAPACITY: usize = 2048;
const EVENT_BROADCAST_CAPACITY: usize = 512;

/// 单轮运行的有界事件日志与实时广播。
#[derive(Clone)]
pub(crate) struct EventJournal {
    inner: Arc<Mutex<JournalInner>>,
    sender: broadcast::Sender<WebEvent>,
    path: Option<PathBuf>,
}

struct JournalInner {
    next_sequence: u64,
    events: VecDeque<WebEvent>,
}

impl EventJournal {
    /// 创建空事件日志。
    ///
    /// 返回:
    /// - 事件日志
    pub(crate) fn new() -> Self {
        let (sender, _) = broadcast::channel(EVENT_BROADCAST_CAPACITY);
        Self {
            inner: Arc::new(Mutex::new(JournalInner {
                next_sequence: 1,
                events: VecDeque::new(),
            })),
            sender,
            path: None,
        }
    }

    /// 从持久化事件文件创建日志。
    ///
    /// 参数:
    /// - `path`: JSONL 事件文件
    ///
    /// 返回:
    /// - 已恢复历史事件的日志
    pub(crate) fn persistent(path: PathBuf) -> Self {
        let journal = Self::new();
        let events = std::fs::read_to_string(&path)
            .ok()
            .into_iter()
            .flat_map(|content| content.lines().map(str::to_string).collect::<Vec<_>>())
            .filter_map(|line| serde_json::from_str::<WebEvent>(&line).ok())
            .collect::<Vec<_>>();
        {
            let mut inner = journal
                .inner
                .lock()
                .unwrap_or_else(|error| error.into_inner());
            inner.next_sequence = events.last().map(|event| event.sequence + 1).unwrap_or(1);
            inner.events = events.into_iter().collect();
        }
        Self {
            path: Some(path),
            ..journal
        }
    }

    /// 写入并广播事件。
    ///
    /// 参数:
    /// - `event`: 尚未分配序号的事件
    ///
    /// 返回:
    /// - 已分配序号的事件
    pub(crate) fn publish(&self, mut event: WebEvent) -> WebEvent {
        let mut inner = self.inner.lock().unwrap_or_else(|error| error.into_inner());
        event.sequence = inner.next_sequence;
        inner.next_sequence = inner.next_sequence.saturating_add(1);
        inner.events.push_back(event.clone());
        while inner.events.len() > EVENT_JOURNAL_CAPACITY {
            inner.events.pop_front();
        }
        drop(inner);
        if let Some(path) = &self.path {
            if let Some(parent) = path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            if let Ok(mut file) = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(path)
            {
                let _ = writeln!(
                    file,
                    "{}",
                    serde_json::to_string(&event).unwrap_or_default()
                );
            }
        }
        let _ = self.sender.send(event.clone());
        event
    }

    /// 返回指定序号之后的历史事件。
    ///
    /// 参数:
    /// - `after`: 已接收的最后事件序号
    ///
    /// 返回:
    /// - 需要补发的事件
    pub(crate) fn events_after(&self, after: u64) -> Vec<WebEvent> {
        let inner = self.inner.lock().unwrap_or_else(|error| error.into_inner());
        inner
            .events
            .iter()
            .filter(|event| event.sequence > after)
            .cloned()
            .collect()
    }

    /// 订阅实时事件。
    ///
    /// 返回:
    /// - 广播接收器
    pub(crate) fn subscribe(&self) -> broadcast::Receiver<WebEvent> {
        self.sender.subscribe()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// 验证事件日志在重新打开后仍能按序号补发。
    #[test]
    fn persistent_journal_replays_after_restart() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("run.jsonl");
        let journal = EventJournal::persistent(path.clone());
        journal.publish(WebEvent::new(
            "run",
            "workspace",
            "session",
            "message.content.delta",
            json!({ "text": "hello" }),
        ));

        let reopened = EventJournal::persistent(path);
        let events = reopened.events_after(0);

        assert_eq!(events.len(), 1);
        assert_eq!(events[0].kind, "message.content.delta");
        assert_eq!(events[0].payload["text"], "hello");
    }
}
