use super::manager::{ActiveRunInfo, StartRunRequest};
use crate::paths::SaiPaths;
use crate::web::workspaces::WorkspaceInfo;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

const CHECKPOINT_FILE: &str = "web/run-checkpoints.json";

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
        Ok(Self {
            path,
            event_dir: paths.state_dir.join("web/run-events"),
            records: Arc::new(Mutex::new(records)),
        })
    }

    /// 新增或替换运行检查点。
    pub(crate) fn upsert(&self, mut checkpoint: RunCheckpoint) -> Result<()> {
        checkpoint.updated_at = chrono::Utc::now().to_rfc3339();
        let mut records = self
            .records
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        records.retain(|record| record.info.run_id != checkpoint.info.run_id);
        records.push(checkpoint);
        self.save_locked(&records)
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
        }
        self.save_locked(&records)
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
        }
        self.save_locked(&records)
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
        if recovery.is_some() {
            self.save_locked(&records)?;
        }
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
        }
        self.save_locked(&records)?;
        Ok(recovered)
    }

    /// 返回指定运行的事件日志文件。
    pub(crate) fn event_path(&self, run_id: &str) -> PathBuf {
        self.event_dir.join(format!("{run_id}.jsonl"))
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::web::runs::RunKind;
    use std::path::PathBuf;

    /// 创建运行检查点测试路径。
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
}
