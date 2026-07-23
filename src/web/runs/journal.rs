use super::WebEvent;
use std::collections::VecDeque;
use std::io::{BufRead, BufReader, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use tokio::sync::broadcast;

const EVENT_JOURNAL_CAPACITY: usize = 2048;
const EVENT_BROADCAST_CAPACITY: usize = 512;
const EVENT_JOURNAL_MAX_BYTES: u64 = 16 * 1024 * 1024;
const EVENT_JOURNAL_COMPACT_EVENTS: usize = EVENT_JOURNAL_CAPACITY * 2;

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
    retained_bytes: usize,
    persisted_bytes: u64,
    persisted_events: usize,
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
                retained_bytes: 0,
                persisted_bytes: 0,
                persisted_events: 0,
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
        let loaded = load_recent_events(&path);
        {
            let mut inner = journal
                .inner
                .lock()
                .unwrap_or_else(|error| error.into_inner());
            inner.next_sequence = loaded
                .events
                .back()
                .map(|event| event.sequence.saturating_add(1))
                .unwrap_or(1);
            inner.events = loaded.events;
            inner.retained_bytes = loaded.retained_bytes;
            inner.persisted_bytes = loaded.source_bytes;
            inner.persisted_events = inner.events.len();
            if loaded.truncated {
                if let Ok(bytes) = rewrite_events(&path, &inner.events) {
                    inner.persisted_bytes = bytes;
                    inner.persisted_events = inner.events.len();
                }
            }
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
        let encoded = encode_bounded_event(&mut event);
        let encoded_bytes = encoded.len().saturating_add(1);
        inner.events.push_back(event.clone());
        inner.retained_bytes = inner.retained_bytes.saturating_add(encoded_bytes);
        trim_retained_events(&mut inner);
        if let Some(path) = &self.path {
            persist_event(path, &encoded, &mut inner);
        }
        drop(inner);
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

/// 从 JSONL 尾部回载有界事件。
struct JournalLoad {
    events: VecDeque<WebEvent>,
    retained_bytes: usize,
    source_bytes: u64,
    truncated: bool,
}

/// 从事件文件尾部读取最近记录，避免重启时把整个日志读入内存。
///
/// 参数:
/// - `path`: JSONL 事件文件路径
///
/// 返回:
/// - 有界事件集合与原文件统计
fn load_recent_events(path: &Path) -> JournalLoad {
    let Ok(mut file) = std::fs::File::open(path) else {
        return JournalLoad {
            events: VecDeque::new(),
            retained_bytes: 0,
            source_bytes: 0,
            truncated: false,
        };
    };
    let source_bytes = file.metadata().map(|metadata| metadata.len()).unwrap_or(0);
    let start = source_bytes.saturating_sub(EVENT_JOURNAL_MAX_BYTES);
    if file.seek(SeekFrom::Start(start)).is_err() {
        return JournalLoad {
            events: VecDeque::new(),
            retained_bytes: 0,
            source_bytes,
            truncated: start > 0,
        };
    }
    let mut reader = BufReader::new(file.take(EVENT_JOURNAL_MAX_BYTES));
    let mut truncated = start > 0;
    if start > 0 {
        let mut partial = Vec::new();
        let _ = reader.read_until(b'\n', &mut partial);
    }
    let mut events = VecDeque::new();
    let mut retained_bytes = 0usize;
    for line in reader.lines().map_while(Result::ok) {
        let Ok(event) = serde_json::from_str::<WebEvent>(&line) else {
            continue;
        };
        retained_bytes = retained_bytes.saturating_add(encoded_event_len(&event));
        events.push_back(event);
        while events.len() > EVENT_JOURNAL_CAPACITY
            || retained_bytes > EVENT_JOURNAL_MAX_BYTES as usize
        {
            if let Some(removed) = events.pop_front() {
                retained_bytes = retained_bytes.saturating_sub(encoded_event_len(&removed));
                truncated = true;
            }
        }
    }
    JournalLoad {
        events,
        retained_bytes,
        source_bytes,
        truncated,
    }
}

/// 将超大事件替换为有界的截断说明。
///
/// 参数:
/// - `event`: 已分配序号的事件
///
/// 返回:
/// - 不超过单轮日志上限的 JSON 文本
fn encode_bounded_event(event: &mut WebEvent) -> String {
    let encoded = serde_json::to_string(event).unwrap_or_default();
    if encoded.len().saturating_add(1) <= EVENT_JOURNAL_MAX_BYTES as usize {
        return encoded;
    }
    let original_kind = std::mem::replace(&mut event.kind, "run.event.truncated".to_string());
    event.payload = serde_json::json!({
        "truncated": true,
        "original_type": original_kind,
        "original_bytes": encoded.len().saturating_add(1),
        "detail": "The event exceeded the retained journal size limit."
    });
    serde_json::to_string(event).unwrap_or_default()
}

/// 追加单条事件，并在达到数量或字节上限后压缩文件。
///
/// 参数:
/// - `path`: JSONL 事件文件路径
/// - `encoded`: 已序列化事件
/// - `inner`: 当前事件日志状态
///
/// 返回:
/// - 无
fn persist_event(path: &Path, encoded: &str, inner: &mut JournalInner) {
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let appended = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .and_then(|mut file| writeln!(file, "{encoded}"));
    if appended.is_err() {
        return;
    }
    inner.persisted_bytes = inner
        .persisted_bytes
        .saturating_add(encoded.len().saturating_add(1) as u64);
    inner.persisted_events = inner.persisted_events.saturating_add(1);
    if (inner.persisted_bytes > EVENT_JOURNAL_MAX_BYTES
        || inner.persisted_events > EVENT_JOURNAL_COMPACT_EVENTS)
        && !inner.events.is_empty()
    {
        if let Ok(bytes) = rewrite_events(path, &inner.events) {
            inner.persisted_bytes = bytes;
            inner.persisted_events = inner.events.len();
        }
    }
}

/// 原子重写当前保留的事件集合。
///
/// 参数:
/// - `path`: JSONL 事件文件路径
/// - `events`: 当前保留事件
///
/// 返回:
/// - 重写后的文件字节数
fn rewrite_events(path: &Path, events: &VecDeque<WebEvent>) -> std::io::Result<u64> {
    let parent = path.parent().unwrap_or_else(|| std::path::Path::new("."));
    std::fs::create_dir_all(parent)?;
    let mut temp = tempfile::NamedTempFile::new_in(parent)?;
    let mut bytes = 0u64;
    for event in events {
        let encoded = serde_json::to_string(event).unwrap_or_default();
        writeln!(temp, "{encoded}")?;
        bytes = bytes.saturating_add(encoded.len().saturating_add(1) as u64);
    }
    temp.persist(path).map_err(|error| error.error)?;
    Ok(bytes)
}

/// 按事件数量和序列化字节数裁剪内存历史。
///
/// 参数:
/// - `inner`: 当前事件日志状态
///
/// 返回:
/// - 无
fn trim_retained_events(inner: &mut JournalInner) {
    while inner.events.len() > EVENT_JOURNAL_CAPACITY
        || inner.retained_bytes > EVENT_JOURNAL_MAX_BYTES as usize
    {
        if let Some(removed) = inner.events.pop_front() {
            inner.retained_bytes = inner
                .retained_bytes
                .saturating_sub(encoded_event_len(&removed));
        }
    }
}

/// 返回事件在 JSONL 中占用的字节数。
///
/// 参数:
/// - `event`: 待计算事件
///
/// 返回:
/// - JSONL 编码字节数
fn encoded_event_len(event: &WebEvent) -> usize {
    serde_json::to_string(event)
        .map(|encoded| encoded.len().saturating_add(1))
        .unwrap_or(1)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// 创建带指定序号的测试事件。
    ///
    /// 参数:
    /// - `sequence`: 事件序号
    ///
    /// 返回:
    /// - 测试事件
    fn event(sequence: u64) -> WebEvent {
        let mut event = WebEvent::new(
            "run",
            "workspace",
            "session",
            "message.content.delta",
            json!({ "text": sequence.to_string() }),
        );
        event.sequence = sequence;
        event
    }

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

    #[test]
    fn persistent_journal_reloads_only_recent_event_capacity() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("run.jsonl");
        let mut file = std::fs::File::create(&path).unwrap();
        for sequence in 1..=(EVENT_JOURNAL_CAPACITY as u64 + 5) {
            writeln!(file, "{}", serde_json::to_string(&event(sequence)).unwrap()).unwrap();
        }
        drop(file);

        let reopened = EventJournal::persistent(path.clone());
        let events = reopened.events_after(0);

        assert_eq!(events.len(), EVENT_JOURNAL_CAPACITY);
        assert_eq!(events.first().unwrap().sequence, 6);
        assert_eq!(
            std::fs::read_to_string(path).unwrap().lines().count(),
            EVENT_JOURNAL_CAPACITY
        );
    }

    #[test]
    fn persistent_journal_reads_only_bounded_file_tail() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("run.jsonl");
        let mut file = std::fs::File::create(&path).unwrap();
        file.write_all(&vec![b'x'; EVENT_JOURNAL_MAX_BYTES as usize + 1024])
            .unwrap();
        writeln!(file).unwrap();
        writeln!(file, "{}", serde_json::to_string(&event(42)).unwrap()).unwrap();
        drop(file);

        let reopened = EventJournal::persistent(path.clone());
        let events = reopened.events_after(0);

        assert_eq!(events.len(), 1);
        assert_eq!(events[0].sequence, 42);
        assert!(std::fs::metadata(path).unwrap().len() < EVENT_JOURNAL_MAX_BYTES);
    }

    #[test]
    fn persistent_journal_compacts_after_append_threshold() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("run.jsonl");
        let journal = EventJournal::persistent(path.clone());
        journal.publish(event(0));
        {
            let mut inner = journal
                .inner
                .lock()
                .unwrap_or_else(|error| error.into_inner());
            inner.persisted_events = EVENT_JOURNAL_COMPACT_EVENTS;
        }

        journal.publish(event(0));

        assert_eq!(std::fs::read_to_string(path).unwrap().lines().count(), 2);
    }

    #[test]
    fn oversized_single_event_is_replaced_with_bounded_notice() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("run.jsonl");
        let journal = EventJournal::persistent(path.clone());

        let published = journal.publish(WebEvent::new(
            "run",
            "workspace",
            "session",
            "tool.result",
            json!({ "output": "x".repeat(EVENT_JOURNAL_MAX_BYTES as usize + 1) }),
        ));

        assert_eq!(published.kind, "run.event.truncated");
        assert_eq!(published.payload["truncated"], true);
        assert!(std::fs::metadata(path).unwrap().len() <= EVENT_JOURNAL_MAX_BYTES);
        let inner = journal
            .inner
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        assert!(inner.retained_bytes <= EVENT_JOURNAL_MAX_BYTES as usize);
    }
}
