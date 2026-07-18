use super::subagent_state::SubagentSnapshot;
use crate::paths::SaiPaths;
use crate::runtime_recovery::{
    NewRuntimeProcessEventInput, NewRuntimeProcessRecord, OwnerKind, ProcessKind,
    RuntimeProcessStatus,
};
use crate::state::StateStore;
use anyhow::Result;

/// 记录子智能体启动到 Runtime Recovery。
///
/// 参数:
/// - `paths`: Sai 路径
/// - `subagent`: 子智能体快照
///
/// 返回:
/// - 记录是否成功
pub(crate) fn record_subagent_started(
    paths: &SaiPaths,
    session_id: &str,
    subagent: &SubagentSnapshot,
) -> Result<()> {
    let state = StateStore::for_session(paths, session_id)?;
    state.record_runtime_process(runtime_process(
        state.session_id(),
        subagent,
        RuntimeProcessStatus::Running,
    ))?;
    state.append_runtime_process_event(NewRuntimeProcessEventInput {
        process_id: runtime_process_id(&subagent.id),
        stream: "lifecycle".to_string(),
        event_kind: "started".to_string(),
        payload_ref: None,
        payload_preview: format!("subagent started: {}", subagent.description),
    })?;
    Ok(())
}

/// 记录子智能体结束到 Runtime Recovery。
///
/// 参数:
/// - `paths`: Sai 路径
/// - `subagent`: 子智能体快照
///
/// 返回:
/// - 记录是否成功
pub(crate) fn record_subagent_finished(
    paths: &SaiPaths,
    session_id: &str,
    subagent: &SubagentSnapshot,
) -> Result<()> {
    let state = StateStore::for_session(paths, session_id)?;
    let status = runtime_status_from_subagent(&subagent.status);
    let seq = state.append_runtime_process_event(NewRuntimeProcessEventInput {
        process_id: runtime_process_id(&subagent.id),
        stream: "lifecycle".to_string(),
        event_kind: subagent.status.clone(),
        payload_ref: None,
        payload_preview: lifecycle_preview(subagent),
    })?;
    let mut process = runtime_process(state.session_id(), subagent, status);
    process.last_seq = seq;
    state.record_runtime_process(process)?;
    Ok(())
}

/// 创建子智能体运行时进程记录。
///
/// 参数:
/// - `subagent`: 子智能体快照
/// - `status`: 运行时进程状态
///
/// 返回:
/// - 运行时进程记录
fn runtime_process(
    session_id: &str,
    subagent: &SubagentSnapshot,
    status: RuntimeProcessStatus,
) -> NewRuntimeProcessRecord {
    NewRuntimeProcessRecord {
        id: runtime_process_id(&subagent.id),
        session_id: session_id.to_string(),
        owner_kind: OwnerKind::Subagent,
        owner_id: subagent.id.clone(),
        process_kind: ProcessKind::Subagent,
        command: subagent.description.clone(),
        cwd: crate::runtime_cwd::current_dir()
            .map(|path| path.display().to_string())
            .unwrap_or_else(|_| ".".to_string()),
        pid: Some(i64::from(std::process::id())),
        pgid: None,
        status,
        last_seq: 0,
    }
}

/// 生成子智能体运行时进程标识。
///
/// 参数:
/// - `subagent_id`: 子智能体 ID
///
/// 返回:
/// - 运行时进程标识
fn runtime_process_id(subagent_id: &str) -> String {
    format!("subagent_{subagent_id}")
}

/// 将子智能体状态映射为运行时进程状态。
///
/// 参数:
/// - `status`: 子智能体状态
///
/// 返回:
/// - 运行时进程状态
fn runtime_status_from_subagent(status: &str) -> RuntimeProcessStatus {
    match status {
        "completed" => RuntimeProcessStatus::Exited,
        "cancelled" => RuntimeProcessStatus::Stopped,
        "failed" => RuntimeProcessStatus::Failed,
        _ => RuntimeProcessStatus::Running,
    }
}

/// 生成生命周期事件预览。
///
/// 参数:
/// - `subagent`: 子智能体快照
///
/// 返回:
/// - 生命周期事件预览
fn lifecycle_preview(subagent: &SubagentSnapshot) -> String {
    subagent.error.clone().unwrap_or_else(|| {
        format!(
            "subagent {}: {}",
            subagent.status,
            subagent.result.as_deref().unwrap_or(&subagent.description)
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::paths::SaiPaths;
    use crate::tools::subagent_state::SubagentSnapshot;
    use std::path::PathBuf;

    fn test_paths(state_dir: PathBuf) -> SaiPaths {
        SaiPaths {
            config_dir: PathBuf::new(),
            config_file: PathBuf::new(),
            secrets_file: PathBuf::new(),
            skills_dir: PathBuf::new(),
            data_dir: PathBuf::new(),
            cache_dir: PathBuf::new(),
            state_dir,
            pictures_dir: PathBuf::new(),
            fish_hook_file: PathBuf::new(),
            bash_hook_file: PathBuf::new(),
            zsh_hook_file: PathBuf::new(),
            powershell_hook_file: PathBuf::new(),
        }
    }

    fn subagent(status: &str) -> SubagentSnapshot {
        SubagentSnapshot {
            id: "subagent_1".to_string(),
            description: "inspect runtime recovery".to_string(),
            subagent_type: "explore".to_string(),
            status: status.to_string(),
            max_steps: 3,
            started_at: 1,
            updated_at: 1,
            step: 0,
            phase: None,
            last_tool: None,
            result: None,
            error: None,
            stats: None,
            worktree_root: None,
            worktree_branch: None,
            parent_workdir: None,
            worktree_merge: None,
        }
    }

    #[test]
    fn records_subagent_started_runtime_process() {
        let temp = tempfile::tempdir().unwrap();
        let paths = test_paths(temp.path().to_path_buf());
        let session_id = StateStore::new(&paths).unwrap().session_id().to_string();

        record_subagent_started(&paths, &session_id, &subagent("running")).unwrap();

        let db_path = crate::state::active_state_dir(&paths)
            .unwrap()
            .join("conversation.db");
        let conn = rusqlite::Connection::open(db_path).unwrap();
        let (owner_kind, owner_id, process_kind, status, last_seq): (
            String,
            String,
            String,
            String,
            i64,
        ) = conn
            .query_row(
                "SELECT owner_kind, owner_id, process_kind, status, last_seq
                 FROM runtime_processes
                 WHERE id = ?1",
                ["subagent_subagent_1"],
                |row| {
                    Ok((
                        row.get(0)?,
                        row.get(1)?,
                        row.get(2)?,
                        row.get(3)?,
                        row.get(4)?,
                    ))
                },
            )
            .unwrap();

        assert_eq!(owner_kind, "subagent");
        assert_eq!(owner_id, "subagent_1");
        assert_eq!(process_kind, "subagent");
        assert_eq!(status, "running");
        assert_eq!(last_seq, 1);
    }
}
