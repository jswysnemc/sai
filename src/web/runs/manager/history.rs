use super::RunManager;
use crate::web::runs::EventJournal;
use anyhow::Result;
use std::collections::{HashMap, HashSet, VecDeque};

/// 保存近期运行对应的内存事件日志。
#[derive(Default)]
pub(super) struct RunJournals {
    pub(super) entries: HashMap<String, EventJournal>,
    pub(super) order: VecDeque<String>,
}

impl RunManager {
    /// 返回指定运行事件日志。
    ///
    /// 参数:
    /// - `run_id`: 运行 ID
    ///
    /// 返回:
    /// - 已加载或可从磁盘恢复的事件日志
    pub(crate) async fn journal(&self, run_id: &str) -> Option<EventJournal> {
        if let Some(journal) = self.journals.read().await.entries.get(run_id).cloned() {
            return Some(journal);
        }
        self.checkpoints.get(run_id)?;
        let loaded = EventJournal::persistent(self.checkpoints.event_path(run_id));
        let mut journals = self.journals.write().await;
        if let Some(journal) = journals.entries.get(run_id).cloned() {
            return Some(journal);
        }
        insert_shared_journal(&mut journals, run_id.to_string(), loaded.clone());
        retain_checkpoint_journals(self, &mut journals);
        Some(loaded)
    }

    /// 删除指定会话的运行检查点、磁盘日志和内存日志。
    ///
    /// 参数:
    /// - `workspace_id`: 会话所属工作区标识
    /// - `session_id`: 会话标识
    ///
    /// 返回:
    /// - 清理结果
    pub(crate) async fn remove_session_history(
        &self,
        workspace_id: &str,
        session_id: &str,
    ) -> Result<()> {
        let removed = self.checkpoints.remove_session(workspace_id, session_id)?;
        if removed.is_empty() {
            return Ok(());
        }
        let removed = removed.into_iter().collect::<HashSet<_>>();
        let mut journals = self.journals.write().await;
        journals
            .entries
            .retain(|run_id, _| !removed.contains(run_id));
        journals.order.retain(|run_id| !removed.contains(run_id));
        Ok(())
    }

    /// 保存运行事件日志，并移除已经没有检查点的终态日志。
    ///
    /// 参数:
    /// - `run_id`: 运行 ID
    /// - `journal`: 运行事件日志
    ///
    /// 返回:
    /// - 无
    pub(super) async fn insert_journal(&self, run_id: String, journal: EventJournal) {
        let mut journals = self.journals.write().await;
        insert_shared_journal(&mut journals, run_id, journal);
        retain_checkpoint_journals(self, &mut journals);
    }
}

/// 将事件日志写入共享索引，并避免同一运行出现重复顺序项。
///
/// 参数:
/// - `journals`: 内存日志索引
/// - `run_id`: 运行标识
/// - `journal`: 事件日志
///
/// 返回:
/// - 无
fn insert_shared_journal(journals: &mut RunJournals, run_id: String, journal: EventJournal) {
    journals.order.retain(|existing| existing != &run_id);
    journals.entries.insert(run_id.clone(), journal);
    journals.order.push_back(run_id);
}

/// 仅删除已经被检查点容量策略淘汰的内存日志。
///
/// 参数:
/// - `manager`: 运行管理器
/// - `journals`: 内存日志索引
///
/// 返回:
/// - 无
fn retain_checkpoint_journals(manager: &RunManager, journals: &mut RunJournals) {
    journals
        .order
        .retain(|run_id| manager.checkpoints.get(run_id).is_some());
    let retained = journals.order.iter().cloned().collect::<HashSet<_>>();
    journals
        .entries
        .retain(|run_id, _| retained.contains(run_id));
}
