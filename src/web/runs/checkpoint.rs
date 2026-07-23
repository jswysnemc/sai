use super::manager::{ActiveRunInfo, StartRunRequest};
use crate::paths::SaiPaths;
use crate::web::workspaces::WorkspaceInfo;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

const CHECKPOINT_FILE: &str = "web/run-checkpoints.json";
pub(super) const RUN_HISTORY_CAPACITY: usize = 32;

/// Web 运行检查点状态。
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum RunCheckpointStatus {
    Queued,
    Running,
    Completed,
    Interrupted,
    Failed,
}

/// 可在进程重启后恢复的运行检查点。
#[derive(Clone, Debug, Deserialize, Serialize)]
pub(crate) struct RunCheckpoint {
    pub(crate) info: ActiveRunInfo,
    pub(crate) workspace: WorkspaceInfo,
    pub(crate) request: StartRunRequest,
    pub(crate) status: RunCheckpointStatus,
    pub(crate) updated_at: String,
}

/// 保存运行请求、队列状态和终态。
#[derive(Clone)]
pub(crate) struct RunCheckpointStore {
    path: PathBuf,
    event_dir: PathBuf,
    records: Arc<Mutex<Vec<RunCheckpoint>>>,
}

impl RunCheckpointStore {
    /// 读取运行检查点存储。
    ///
    /// 参数:
    /// - `paths`: Sai 路径集合
    ///
    /// 返回:
    /// - 运行检查点存储
    pub(crate) fn new(paths: &SaiPaths) -> Result<Self> {
        let path = paths.state_dir.join(CHECKPOINT_FILE);
        let records = if path.is_file() {
            serde_json::from_slice(&std::fs::read(&path)?)?
        } else {
            Vec::new()
        };
        let store = Self {
            path,
            event_dir: paths.state_dir.join("web/run-events"),
            records: Arc::new(Mutex::new(records)),
        };
        store.prune()?;
        store.remove_orphan_journals()?;
        Ok(store)
    }

    /// 新增或替换运行检查点。
    pub(crate) fn upsert(&self, mut checkpoint: RunCheckpoint) -> Result<()> {
        checkpoint.updated_at = chrono::Utc::now().to_rfc3339();
        compact_terminal_checkpoint(&mut checkpoint);
        let mut records = self
            .records
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        records.retain(|record| record.info.run_id != checkpoint.info.run_id);
        records.push(checkpoint);
        let removed = prune_terminal_records(&mut records);
        self.save_locked(&records)?;
        drop(records);
        self.remove_journals(&removed)
    }

    /// 更新指定运行状态。
    pub(crate) fn update_status(&self, run_id: &str, status: RunCheckpointStatus) -> Result<()> {
        let mut records = self
            .records
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        if let Some(record) = records
            .iter_mut()
            .find(|record| record.info.run_id == run_id)
        {
            record.status = status;
            record.info.status = status;
            record.updated_at = chrono::Utc::now().to_rfc3339();
            compact_terminal_checkpoint(record);
        }
        let removed = prune_terminal_records(&mut records);
        self.save_locked(&records)?;
        drop(records);
        self.remove_journals(&removed)
    }

    /// 将运行标记为中断并保存输入恢复信息。
    ///
    /// 参数:
    /// - `run_id`: 运行标识
    /// - `discard_user_turn`: 是否撤销用户气泡
    /// - `restore_input`: 可选待恢复输入
    ///
    /// 返回:
    /// - 更新是否成功
    pub(crate) fn update_interruption(
        &self,
        run_id: &str,
        discard_user_turn: bool,
        restore_input: Option<String>,
    ) -> Result<()> {
        let mut records = self
            .records
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        if let Some(record) = records
            .iter_mut()
            .find(|record| record.info.run_id == run_id)
        {
            record.status = RunCheckpointStatus::Interrupted;
            record.info.status = RunCheckpointStatus::Interrupted;
            record.info.discard_user_turn = discard_user_turn;
            record.info.restore_input = restore_input;
            record.updated_at = chrono::Utc::now().to_rfc3339();
            compact_terminal_checkpoint(record);
        }
        let removed = prune_terminal_records(&mut records);
        self.save_locked(&records)?;
        drop(records);
        self.remove_journals(&removed)
    }

    /// 读取并消费指定会话的无回复中断恢复输入。
    ///
    /// 参数:
    /// - `workspace_id`: 工作区标识
    /// - `session_id`: 会话标识
    ///
    /// 返回:
    /// - 待恢复运行信息
    pub(crate) fn take_interruption_recovery(
        &self,
        workspace_id: &str,
        session_id: &str,
    ) -> Result<Option<ActiveRunInfo>> {
        let mut records = self
            .records
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let recovery = records
            .iter_mut()
            .rev()
            .find(|record| {
                record.info.workspace_id == workspace_id
                    && record.info.session_id == session_id
                    && record.info.discard_user_turn
                    && record.info.restore_input.is_some()
            })
            .map(|record| {
                let recovery = record.info.clone();
                record.info.discard_user_turn = false;
                record.info.restore_input = None;
                record.updated_at = chrono::Utc::now().to_rfc3339();
                recovery
            });
        let removed = if recovery.is_some() {
            compact_terminal_records(&mut records);
            let removed = prune_terminal_records(&mut records);
            self.save_locked(&records)?;
            removed
        } else {
            Vec::new()
        };
        drop(records);
        self.remove_journals(&removed)?;
        Ok(recovery)
    }

    /// 返回指定运行检查点。
    pub(crate) fn get(&self, run_id: &str) -> Option<RunCheckpoint> {
        self.records
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .iter()
            .find(|record| record.info.run_id == run_id)
            .cloned()
    }

    /// 返回等待恢复的排队运行。
    pub(crate) fn queued(&self) -> Vec<RunCheckpoint> {
        self.records
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .iter()
            .filter(|record| record.status == RunCheckpointStatus::Queued)
            .cloned()
            .collect()
    }

    /// 将进程退出时仍在运行的检查点恢复为中断状态。
    pub(crate) fn recover_running_as_interrupted(&self) -> Result<Vec<RunCheckpoint>> {
        let mut records = self
            .records
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let mut recovered = Vec::new();
        for record in records.iter_mut() {
            if record.status != RunCheckpointStatus::Running {
                continue;
            }
            record.status = RunCheckpointStatus::Interrupted;
            record.info.status = RunCheckpointStatus::Interrupted;
            record.updated_at = chrono::Utc::now().to_rfc3339();
            recovered.push(record.clone());
            compact_terminal_checkpoint(record);
        }
        let removed = prune_terminal_records(&mut records);
        self.save_locked(&records)?;
        drop(records);
        self.remove_journals(&removed)?;
        Ok(recovered)
    }

    /// 返回指定运行的事件日志文件。
    pub(crate) fn event_path(&self, run_id: &str) -> PathBuf {
        self.event_dir.join(format!("{run_id}.jsonl"))
    }

    /// 删除指定会话的全部终态运行记录和事件日志。
    ///
    /// 参数:
    /// - `workspace_id`: 会话所属工作区标识
    /// - `session_id`: 会话标识
    ///
    /// 返回:
    /// - 被删除的运行标识
    pub(crate) fn remove_session(
        &self,
        workspace_id: &str,
        session_id: &str,
    ) -> Result<Vec<String>> {
        let mut records = self
            .records
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let removed = records
            .iter()
            .filter(|record| {
                record.info.workspace_id == workspace_id && record.info.session_id == session_id
            })
            .map(|record| record.info.run_id.clone())
            .collect::<Vec<_>>();
        if removed.is_empty() {
            return Ok(removed);
        }
        let removed_ids = removed.iter().collect::<HashSet<_>>();
        records.retain(|record| !removed_ids.contains(&record.info.run_id));
        self.save_locked(&records)?;
        drop(records);
        self.remove_journals(&removed)?;
        Ok(removed)
    }

    /// 清理终态检查点中的大字段，并淘汰超过保留上限的记录。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 检查点保存与日志清理结果
    fn prune(&self) -> Result<()> {
        let mut records = self
            .records
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let compacted = compact_terminal_records(&mut records);
        let removed = prune_terminal_records(&mut records);
        if !compacted && removed.is_empty() {
            return Ok(());
        }
        self.save_locked(&records)?;
        drop(records);
        self.remove_journals(&removed)
    }

    /// 删除一组运行对应的事件日志。
    ///
    /// 参数:
    /// - `run_ids`: 待删除日志对应的运行标识
    ///
    /// 返回:
    /// - 全部日志删除结果
    fn remove_journals(&self, run_ids: &[String]) -> Result<()> {
        for run_id in run_ids {
            let path = self.event_path(run_id);
            match std::fs::remove_file(path) {
                Ok(()) => {}
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => return Err(error.into()),
            }
        }
        Ok(())
    }

    /// 删除没有对应检查点的遗留事件日志。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 遗留日志扫描与删除结果
    fn remove_orphan_journals(&self) -> Result<()> {
        if !self.event_dir.is_dir() {
            return Ok(());
        }
        let retained = self
            .records
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .iter()
            .map(|record| record.info.run_id.clone())
            .collect::<HashSet<_>>();
        for entry in std::fs::read_dir(&self.event_dir)? {
            let entry = entry?;
            let path = entry.path();
            if !entry.file_type()?.is_file()
                || path.extension().and_then(|value| value.to_str()) != Some("jsonl")
            {
                continue;
            }
            let Some(run_id) = path.file_stem().and_then(|value| value.to_str()) else {
                continue;
            };
            if !retained.contains(run_id) {
                std::fs::remove_file(path)?;
            }
        }
        Ok(())
    }

    /// 原子保存全部检查点。
    fn save_locked(&self, records: &[RunCheckpoint]) -> Result<()> {
        let parent = self
            .path
            .parent()
            .unwrap_or_else(|| std::path::Path::new("."));
        std::fs::create_dir_all(parent)?;
        let temp = tempfile::NamedTempFile::new_in(parent)?;
        std::fs::write(temp.path(), serde_json::to_vec_pretty(records)?)?;
        temp.persist(&self.path)?;
        Ok(())
    }
}

/// 清理终态检查点中不再参与恢复的大字段。
///
/// 参数:
/// - `records`: 待清理检查点
///
/// 返回:
/// - 是否修改了任一检查点
fn compact_terminal_records(records: &mut [RunCheckpoint]) -> bool {
    let mut changed = false;
    for record in records {
        changed |= compact_terminal_checkpoint(record);
    }
    changed
}

/// 清理单个终态检查点中的输入与图片副本。
///
/// 参数:
/// - `record`: 待清理检查点
///
/// 返回:
/// - 是否修改了检查点
fn compact_terminal_checkpoint(record: &mut RunCheckpoint) -> bool {
    if !is_terminal(record) {
        return false;
    }
    let changed = !record.info.input.is_empty()
        || !record.info.image_urls.is_empty()
        || !record.request.input.is_empty()
        || record.request.image_url.is_some()
        || !record.request.image_urls.is_empty();
    record.info.input.clear();
    record.info.image_urls.clear();
    record.request.input.clear();
    record.request.image_url = None;
    record.request.image_urls.clear();
    changed
}

/// 淘汰最早的终态检查点。
///
/// 参数:
/// - `records`: 按创建顺序保存的检查点
///
/// 返回:
/// - 被淘汰的运行标识
fn prune_terminal_records(records: &mut Vec<RunCheckpoint>) -> Vec<String> {
    let terminal_count = records
        .iter()
        .filter(|record| is_terminal(record))
        .count();
    let mut remaining = terminal_count.saturating_sub(RUN_HISTORY_CAPACITY);
    if remaining == 0 {
        return Vec::new();
    }
    let mut removed = Vec::with_capacity(remaining);
    records.retain(|record| {
        if remaining > 0 && is_terminal(record) {
            remaining -= 1;
            removed.push(record.info.run_id.clone());
            false
        } else {
            true
        }
    });
    removed
}

/// 判断检查点是否已经终结。
///
/// 参数:
/// - `record`: 待检查运行记录
///
/// 返回:
/// - 是否属于完成、中断或失败状态
fn is_terminal(record: &RunCheckpoint) -> bool {
    matches!(
        record.status,
        RunCheckpointStatus::Completed
            | RunCheckpointStatus::Interrupted
            | RunCheckpointStatus::Failed
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::web::runs::RunKind;
    use std::path::PathBuf;

    /// 创建运行检查点测试路径。
    ///
    /// 参数:
    /// - `root`: 测试状态根目录
    ///
    /// 返回:
    /// - 隔离的 Sai 路径集合
    fn test_paths(root: PathBuf) -> SaiPaths {
        SaiPaths {
            config_dir: root.join("config"),
            config_file: root.join("config/config.jsonc"),
            secrets_file: root.join("config/secrets.jsonc"),
            skills_dir: root.join("config/skills"),
            data_dir: root.join("data"),
            cache_dir: root.join("cache"),
            state_dir: root.join("state"),
            pictures_dir: root.join("pictures"),
            fish_hook_file: root.join("fish/sai.fish"),
            bash_hook_file: root.join("shell/bash-hook.sh"),
            zsh_hook_file: root.join("shell/zsh-hook.zsh"),
            powershell_hook_file: root.join("shell/powershell-hook.ps1"),
        }
    }

    #[test]
    /// 验证服务重启后的无回复输入只能消费一次。
    fn interruption_recovery_is_consumed_once() {
        let temp = tempfile::tempdir().unwrap();
        let store = RunCheckpointStore::new(&test_paths(temp.path().to_path_buf())).unwrap();
        let info = ActiveRunInfo {
            run_id: "run-1".to_string(),
            workspace_id: "workspace".to_string(),
            session_id: "session".to_string(),
            input: "edit me".to_string(),
            image_urls: Vec::new(),
            status: RunCheckpointStatus::Running,
            discard_user_turn: false,
            restore_input: None,
        };
        store
            .upsert(RunCheckpoint {
                info: info.clone(),
                workspace: WorkspaceInfo {
                    id: info.workspace_id.clone(),
                    name: "workspace".to_string(),
                    path: temp.path().display().to_string(),
                    last_opened_at: String::new(),
                },
                request: StartRunRequest {
                    kind: RunKind::Conversation,
                    session_id: info.session_id.clone(),
                    input: info.input.clone(),
                    agent_id: None,
                    image_url: None,
                    image_urls: Vec::new(),
                    mode: None,
                    provider_id: None,
                    model: None,
                    thinking_level: None,
                },
                status: RunCheckpointStatus::Running,
                updated_at: String::new(),
            })
            .unwrap();
        store
            .update_interruption("run-1", true, Some("edit me".to_string()))
            .unwrap();

        let first = store
            .take_interruption_recovery("workspace", "session")
            .unwrap();
        let second = store
            .take_interruption_recovery("workspace", "session")
            .unwrap();

        assert_eq!(first.unwrap().restore_input.as_deref(), Some("edit me"));
        assert!(second.is_none());
    }

    /// 创建终态检查点测试数据。
    ///
    /// 参数:
    /// - `root`: 工作区目录
    /// - `run_id`: 运行标识
    ///
    /// 返回:
    /// - 完成状态的检查点
    fn completed_checkpoint(root: &std::path::Path, run_id: &str) -> RunCheckpoint {
        RunCheckpoint {
            info: ActiveRunInfo {
                run_id: run_id.to_string(),
                workspace_id: "workspace".to_string(),
                session_id: "session".to_string(),
                input: String::new(),
                image_urls: Vec::new(),
                status: RunCheckpointStatus::Completed,
                discard_user_turn: false,
                restore_input: None,
            },
            workspace: WorkspaceInfo {
                id: "workspace".to_string(),
                name: "workspace".to_string(),
                path: root.display().to_string(),
                last_opened_at: String::new(),
            },
            request: StartRunRequest {
                kind: RunKind::Conversation,
                session_id: "session".to_string(),
                input: String::new(),
                agent_id: None,
                image_url: None,
                image_urls: Vec::new(),
                mode: None,
                provider_id: None,
                model: None,
                thinking_level: None,
            },
            status: RunCheckpointStatus::Completed,
            updated_at: String::new(),
        }
    }

    #[test]
    fn prunes_old_terminal_checkpoints_and_their_journals() {
        let temp = tempfile::tempdir().unwrap();
        let store = RunCheckpointStore::new(&test_paths(temp.path().to_path_buf())).unwrap();
        for index in 0..=RUN_HISTORY_CAPACITY {
            let run_id = format!("run-{index}");
            let event_path = store.event_path(&run_id);
            std::fs::create_dir_all(event_path.parent().unwrap()).unwrap();
            std::fs::write(&event_path, "event\n").unwrap();
            store
                .upsert(completed_checkpoint(temp.path(), &run_id))
                .unwrap();
        }

        assert!(store.get("run-0").is_none());
        assert!(!store.event_path("run-0").exists());
        assert!(store.get(&format!("run-{RUN_HISTORY_CAPACITY}")).is_some());
        assert!(store
            .event_path(&format!("run-{RUN_HISTORY_CAPACITY}"))
            .exists());
    }

    #[test]
    fn prunes_interrupted_checkpoints_with_pending_restore_input() {
        let temp = tempfile::tempdir().unwrap();
        let store = RunCheckpointStore::new(&test_paths(temp.path().to_path_buf())).unwrap();
        for index in 0..=RUN_HISTORY_CAPACITY {
            let run_id = format!("run-{index}");
            let mut checkpoint = completed_checkpoint(temp.path(), &run_id);
            checkpoint.status = RunCheckpointStatus::Interrupted;
            checkpoint.info.status = RunCheckpointStatus::Interrupted;
            checkpoint.info.discard_user_turn = true;
            checkpoint.info.restore_input = Some(format!("restore-{index}"));
            store.upsert(checkpoint).unwrap();
        }

        assert!(store.get("run-0").is_none());
        assert!(store.get("run-1").is_some());
        assert!(store
            .get(&format!("run-{RUN_HISTORY_CAPACITY}"))
            .is_some());
    }

    #[test]
    fn terminal_checkpoints_discard_input_and_image_payloads() {
        let temp = tempfile::tempdir().unwrap();
        let store = RunCheckpointStore::new(&test_paths(temp.path().to_path_buf())).unwrap();
        let mut checkpoint = completed_checkpoint(temp.path(), "run-large");
        checkpoint.info.input = "large input".to_string();
        checkpoint.info.image_urls = vec!["data:image/png;base64,AAAA".to_string()];
        checkpoint.request.input = "large input".to_string();
        checkpoint.request.image_url = Some("data:image/png;base64,AAAA".to_string());
        checkpoint.request.image_urls = vec!["data:image/png;base64,BBBB".to_string()];

        store.upsert(checkpoint).unwrap();

        let stored = store.get("run-large").unwrap();
        assert!(stored.info.input.is_empty());
        assert!(stored.info.image_urls.is_empty());
        assert!(stored.request.input.is_empty());
        assert!(stored.request.image_url.is_none());
        assert!(stored.request.image_urls.is_empty());
    }

    #[test]
    fn startup_removes_orphan_event_journals() {
        let temp = tempfile::tempdir().unwrap();
        let paths = test_paths(temp.path().to_path_buf());
        let orphan = paths.state_dir.join("web/run-events/orphan.jsonl");
        std::fs::create_dir_all(orphan.parent().unwrap()).unwrap();
        std::fs::write(&orphan, "event\n").unwrap();

        let _store = RunCheckpointStore::new(&paths).unwrap();

        assert!(!orphan.exists());
    }
}
