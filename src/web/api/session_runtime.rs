use super::super::error::{WebError, WebResult};
use crate::paths::SaiPaths;
use crate::tools::subagent_state::{clear_subagents_for_owner, list_subagents_for_owner};

/// 校验指定会话没有运行中的子智能体，并返回其稳定作用域键。
///
/// 参数:
/// - `paths`: Sai 路径集合
/// - `session_id`: 会话标识
///
/// 返回:
/// - 会话状态目录对应的稳定作用域键
pub(super) fn reject_running_subagents(paths: &SaiPaths, session_id: &str) -> WebResult<String> {
    let (_, state_dir) = crate::state::locate_session_dirs(paths, session_id)
        .map_err(|error| WebError::not_found(error.to_string()))?;
    let owner_key = state_dir.display().to_string();
    if list_subagents_for_owner(&owner_key)
        .iter()
        .any(|subagent| subagent.status == "running")
    {
        return Err(WebError::conflict(
            "stop running subagents before modifying session data",
        ));
    }
    Ok(owner_key)
}

/// 校验指定工作区中的会话没有运行中的子智能体，并返回其稳定作用域键。
///
/// 参数:
/// - `paths`: Sai 路径集合
/// - `workspace_path`: 会话所属工作区目录
/// - `session_id`: 会话标识
///
/// 返回:
/// - 会话状态目录对应的稳定作用域键
pub(super) fn reject_running_subagents_for_workspace(
    paths: &SaiPaths,
    workspace_path: &std::path::Path,
    session_id: &str,
) -> WebResult<String> {
    let (_, state_dir) =
        crate::state::state_dir_for_workspace_session(paths, workspace_path, session_id)
            .map_err(|error| WebError::not_found(error.to_string()))?;
    let owner_key = state_dir.display().to_string();
    if list_subagents_for_owner(&owner_key)
        .iter()
        .any(|subagent| subagent.status == "running")
    {
        return Err(WebError::conflict(
            "stop running subagents before modifying session data",
        ));
    }
    Ok(owner_key)
}

/// 清除会话删除或重置后不应继续保留的瞬态运行状态。
///
/// 参数:
/// - `owner_key`: 会话状态目录对应的稳定作用域键
/// - `session_id`: 会话标识
///
/// 返回:
/// - 无
pub(super) fn clear_session_runtime_records(owner_key: &str, session_id: &str) {
    // 1. 清除内存与持久化子智能体记录
    clear_subagents_for_owner(owner_key);
    // 2. 撤销仍在等待前端处理的交互请求
    crate::permission::discard_pending_permissions_for_session(session_id);
    crate::question::discard_pending_questions_for_session(session_id);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::subagent_state::{
        create_subagent_for_owner, finish_subagent, list_subagents_for_owner,
    };

    /// 验证会话瞬态状态清理会移除已结束的子智能体记录。
    #[test]
    fn clear_runtime_records_removes_finished_subagents() {
        let temp = tempfile::tempdir().unwrap();
        let owner = temp.path().join("removed-session");
        std::fs::create_dir_all(&owner).unwrap();
        let owner_key = owner.display().to_string();
        let (subagent, _cancel) = create_subagent_for_owner(
            &owner_key,
            "finished task".to_string(),
            "general".to_string(),
            10,
        );
        finish_subagent(
            &subagent.id,
            "completed",
            Some("done".to_string()),
            None,
            None,
        );
        std::fs::remove_dir_all(&owner).unwrap();

        clear_session_runtime_records(&owner_key, "session-test");

        assert!(list_subagents_for_owner(&owner_key).is_empty());
        assert!(!owner.exists());
    }
}
