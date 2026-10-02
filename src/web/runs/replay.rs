use super::WebEvent;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::io::Write;
use std::path::Path;

const MAX_SNAPSHOT_BYTES: usize = 64 * 1024 * 1024;

/// 【会话恢复】【快照载荷】替换客户端运行状态后，从 through_sequence 之后继续接收增量。
#[derive(Clone, Serialize)]
pub(crate) struct ReplayReset {
    pub through_sequence: u64,
    pub events: Vec<WebEvent>,
    pub incomplete_run_ids: Vec<String>,
}

/// 【会话恢复】【补发结果】正常补发与完整状态恢复使用互斥路径。
pub(crate) struct ReplayBatch {
    pub events: Vec<WebEvent>,
    pub reset: Option<ReplayReset>,
    pub through_sequence: u64,
}

/// 【会话恢复】【运行快照】只保存尚未结束的运行，连续文本增量合并成一条事件。
#[derive(Default, Serialize, Deserialize)]
pub(super) struct ReplaySnapshot {
    through_sequence: u64,
    runs: BTreeMap<String, RunReplay>,
    incomplete: BTreeSet<String>,
    #[serde(skip)]
    bytes: usize,
}

#[derive(Default, Serialize, Deserialize)]
struct RunReplay {
    events: Vec<WebEvent>,
    #[serde(skip)]
    bytes: usize,
}

impl ReplaySnapshot {
    /// 【会话恢复】【快照更新】接收已分配序号的事件；参数为事件，无返回值。
    pub(super) fn observe(&mut self, event: &WebEvent) {
        self.observe_bounded(event, MAX_SNAPSHOT_BYTES);
    }

    /// 【会话恢复】【容量控制】合并文本并显式标记无法完整保留的运行。
    /// @param event 为新事件；limit 为快照总字节上限；无返回值
    fn observe_bounded(&mut self, event: &WebEvent, limit: usize) {
        if event.sequence <= self.through_sequence {
            return;
        }
        self.through_sequence = event.sequence;
        if event.run_id.is_empty() {
            return;
        }
        if matches!(
            event.kind.as_str(),
            "run.completed" | "run.failed" | "run.interrupted" | "run.merged"
        ) {
            if let Some(run) = self.runs.remove(&event.run_id) {
                self.bytes = self.bytes.saturating_sub(run.bytes);
            }
            self.incomplete.remove(&event.run_id);
            return;
        }
        if event.kind == "run.event.truncated" {
            self.incomplete.insert(event.run_id.clone());
        }
        let entry = matches!(
            event.kind.as_str(),
            "run.started" | "run.queued" | "run.dequeued"
        );
        if entry && !self.runs.contains_key(&event.run_id) {
            self.runs.insert(event.run_id.clone(), RunReplay::default());
            self.incomplete.remove(&event.run_id);
        }
        let Some(run) = self.runs.get_mut(&event.run_id) else {
            if event.kind.starts_with("message.")
                || event.kind.starts_with("tool.")
                || event.kind.ends_with(".requested")
            {
                self.incomplete.insert(event.run_id.clone());
            }
            return;
        };
        if self.incomplete.contains(&event.run_id) {
            return;
        }
        let added = if let Some(previous) = run
            .events
            .last_mut()
            .filter(|previous| mergeable(previous, event))
        {
            let delta = event.payload["text"].as_str().unwrap_or_default();
            if let Some(serde_json::Value::String(text)) = previous.payload.get_mut("text") {
                text.push_str(delta);
            }
            previous.sequence = event.sequence;
            // 保留第一片时间，避免恢复后首字时间被最后一片覆盖
            serde_json::to_string(delta).map_or(delta.len(), |text| text.len().saturating_sub(2))
        } else {
            let bytes = encoded_len(event);
            run.events.push(event.clone());
            bytes
        };
        run.bytes = run.bytes.saturating_add(added);
        self.bytes = self.bytes.saturating_add(added);
        if self.bytes > limit {
            self.incomplete.insert(event.run_id.clone());
            self.bytes = self.bytes.saturating_sub(run.bytes);
            // 保留运行入口，客户端仍能显示输入和后续状态，但必须提示旧内容不完整
            run.events.truncate(1);
            run.bytes = run.events.iter().map(encoded_len).sum();
            if self.bytes.saturating_add(run.bytes) > limit {
                run.events.clear();
                run.bytes = 0;
            }
            self.bytes = self.bytes.saturating_add(run.bytes);
        }
    }

    /// 【会话恢复】【读取水位】无参数，返回已吸收的最后事件序号。
    pub(super) fn sequence(&self) -> u64 {
        self.through_sequence
    }

    /// 【会话恢复】【历史缺口】旧日志不连续时标记现存运行；无参数或返回值。
    pub(super) fn mark_gap(&mut self) {
        self.incomplete.extend(self.runs.keys().cloned());
    }

    /// 【会话恢复】【快照输出】返回按序排列的运行事件，客户端需整体替换旧状态。
    /// @returns 当前恢复载荷；无参数
    pub(super) fn reset(&self) -> ReplayReset {
        let mut events = self
            .runs
            .values()
            .flat_map(|run| run.events.iter().cloned())
            .collect::<Vec<_>>();
        events.sort_by_key(|event| event.sequence);
        ReplayReset {
            through_sequence: self.through_sequence,
            events,
            incomplete_run_ids: self.incomplete.iter().cloned().collect(),
        }
    }

    /// 【会话恢复】【检查点读取】恢复最近压缩时的快照；参数为日志路径，返回快照。
    pub(super) fn load(journal: &Path) -> Self {
        let mut snapshot: Self = std::fs::read(journal.with_extension("replay.json"))
            .ok()
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .unwrap_or_default();
        for run in snapshot.runs.values_mut() {
            run.bytes = run.events.iter().map(encoded_len).sum();
        }
        snapshot.bytes = snapshot.runs.values().map(|run| run.bytes).sum();
        snapshot
    }

    /// 【会话恢复】【检查点保存】仅在日志压缩时原子保存，不按文本增量重写快照。
    /// @param journal 为日志路径；返回持久化结果
    pub(super) fn save(&self, journal: &Path) -> std::io::Result<()> {
        let path = journal.with_extension("replay.json");
        let parent = path.parent().unwrap_or_else(|| Path::new("."));
        std::fs::create_dir_all(parent)?;
        let mut temporary = tempfile::NamedTempFile::new_in(parent)?;
        serde_json::to_writer(temporary.as_file_mut(), self)?;
        temporary.flush()?;
        temporary.persist(path).map_err(|error| error.error)?;
        Ok(())
    }
}

/// 【会话恢复】【文本合并】仅合并同一运行中相邻的同类文本，保持工具与控制事件顺序。
/// @param previous 为已保存事件；next 为新事件；返回是否可合并
fn mergeable(previous: &WebEvent, next: &WebEvent) -> bool {
    previous.run_id == next.run_id
        && previous.kind == next.kind
        && matches!(
            next.kind.as_str(),
            "message.content.delta" | "message.reasoning.delta" | "compaction.delta"
        )
        && previous.payload["text"].is_string()
        && next.payload["text"].is_string()
}

/// 【会话恢复】【容量估算】参数为事件，返回序列化字节数。
fn encoded_len(event: &WebEvent) -> usize {
    serde_json::to_vec(event).map_or(0, |bytes| bytes.len())
}

#[cfg(test)]
#[path = "replay_tests.rs"]
mod tests;
