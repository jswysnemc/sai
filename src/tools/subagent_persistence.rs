use super::subagent_state::SubagentSnapshot;
use super::subagent_timeline::SubagentTimelineEntry;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

const STATE_FILE: &str = "subagents.json";

/// 可跨进程恢复的子智能体记录。
#[derive(Clone, Deserialize, Serialize)]
pub(crate) struct PersistedSubagent {
    pub(crate) owner_key: String,
    pub(crate) snapshot: SubagentSnapshot,
    pub(crate) timeline: Vec<SubagentTimelineEntry>,
    pub(crate) finish_notified: bool,
}

/// 读取父会话的子智能体记录。
///
/// 参数:
/// - `owner_key`: 父会话状态目录
///
/// 返回:
/// - 已保存的子智能体记录
pub(crate) fn load(owner_key: &str) -> Result<Vec<PersistedSubagent>> {
    let Some(path) = state_file(owner_key) else {
        return Ok(Vec::new());
    };
    if !path.is_file() {
        return Ok(Vec::new());
    }
    Ok(serde_json::from_slice(&std::fs::read(path)?)?)
}

/// 原子保存父会话的子智能体记录。
///
/// 参数:
/// - `owner_key`: 父会话状态目录
/// - `records`: 需要保存的记录
pub(crate) fn save(owner_key: &str, records: &[PersistedSubagent]) -> Result<()> {
    let Some(path) = state_file(owner_key) else {
        return Ok(());
    };
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    std::fs::create_dir_all(parent)?;
    let temp = tempfile::NamedTempFile::new_in(parent)?;
    std::fs::write(temp.path(), serde_json::to_vec_pretty(records)?)?;
    temp.persist(path)?;
    Ok(())
}

/// 返回有效父会话对应的持久化文件。
fn state_file(owner_key: &str) -> Option<PathBuf> {
    let path = PathBuf::from(owner_key);
    path.is_absolute().then(|| path.join(STATE_FILE))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 验证子智能体结果和时间线可以跨进程文件往返。
    #[test]
    fn persists_completion_payload() {
        let temp = tempfile::tempdir().unwrap();
        let owner_key = temp.path().display().to_string();
        let record = PersistedSubagent {
            owner_key: owner_key.clone(),
            snapshot: SubagentSnapshot {
                id: "subagent-1".to_string(),
                description: "inspect".to_string(),
                subagent_type: "general".to_string(),
                status: "completed".to_string(),
                max_steps: 5,
                started_at: 1,
                updated_at: 2,
                step: 1,
                phase: None,
                last_tool: None,
                result: Some("done".to_string()),
                error: None,
                stats: None,
                worktree_root: None,
                worktree_branch: None,
                parent_workdir: None,
                worktree_merge: None,
            },
            timeline: vec![SubagentTimelineEntry::Text {
                text: "done".to_string(),
            }],
            finish_notified: false,
        };

        save(&owner_key, std::slice::from_ref(&record)).unwrap();
        let loaded = load(&owner_key).unwrap();

        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].snapshot.result.as_deref(), Some("done"));
        assert!(!loaded[0].finish_notified);
    }
}
