use crate::paths::SaiPaths;
use crate::runner::{ActiveRunGuard, SessionOwner, SessionPresenceGuard};
use crate::state::{create_session_for_workspace, locate_session_dirs, SessionInfo};
use crate::tools::subagent_persistence::{self, PersistedSubagent};
use crate::tools::subagent_state::{create_subagent_for_owner, SubagentSnapshot};
use crate::tools::ToolRegistry;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

mod agent_probe;
mod messaging;
mod registry;
mod session_control;
mod session_probe;

/// 构造绑定当前会话的探测工具注册表。
///
/// 参数:
/// - `paths`: Sai 路径
/// - `session`: 当前会话
///
/// 返回:
/// - 已注册两个探测工具的注册表
fn registry_for(paths: &SaiPaths, session: &SessionInfo) -> ToolRegistry {
    let state_dir = locate_session_dirs(paths, &session.id).unwrap().1;
    let mut registry = ToolRegistry::new();
    super::register(
        &mut registry,
        paths.clone(),
        state_dir.display().to_string(),
        session.id.clone(),
        false,
    );
    registry
}

/// 调用探测工具并把输出解析为 JSON。
///
/// 参数:
/// - `registry`: 工具注册表
/// - `tool`: 工具名
/// - `arguments`: 原始 JSON 参数
///
/// 返回:
/// - 解析后的工具输出
async fn probe(registry: &ToolRegistry, tool: &str, arguments: &str) -> Value {
    let output = registry.call(tool, arguments).await.unwrap();
    serde_json::from_str(&output).unwrap()
}

/// 在当前工作目录里建一个会话并返回其状态目录。
///
/// 参数:
/// - `paths`: Sai 路径
/// - `workspace`: 工作区目录
/// - `title`: 会话标题
///
/// 返回:
/// - (会话信息, 会话状态目录)
fn session_in(paths: &SaiPaths, workspace: &Path, title: &str) -> (SessionInfo, PathBuf) {
    let session = create_session_for_workspace(paths, workspace, Some(title)).unwrap();
    let state_dir = locate_session_dirs(paths, &session.id).unwrap().1;
    (session, state_dir)
}

/// 在指定会话状态目录里落一份已持久化的子智能体记录。
///
/// 参数:
/// - `state_dir`: 会话状态目录
/// - `id`: 子智能体 ID
///
/// 返回:
/// - 无
fn persist_subagent(state_dir: &Path, id: &str) {
    let owner_key = state_dir.display().to_string();
    subagent_persistence::save(
        &owner_key,
        std::slice::from_ref(&PersistedSubagent {
            owner_key: owner_key.clone(),
            snapshot: SubagentSnapshot {
                id: id.to_string(),
                goal_id: None,
                description: "persisted".to_string(),
                subagent_type: "explore".to_string(),
                status: "completed".to_string(),
                max_steps: 3,
                started_at: 1,
                updated_at: 2,
                step: 2,
                phase: None,
                last_tool: Some("grep".to_string()),
                result: Some("done".to_string()),
                error: None,
                stats: Some(json!({ "total_tokens": 4096 })),
                worktree_root: None,
                worktree_branch: None,
                parent_workdir: None,
                worktree_merge: None,
                persistent: false,
                pending_messages: 0,
                turns_completed: 0,
            },
            timeline: Vec::new(),
            finish_notified: true,
        }),
    )
    .unwrap();
}

/// 构造可切换跨会话开关的工具注册表。
///
/// 参数:
/// - `paths`: Sai 路径
/// - `session`: 当前会话
/// - `cross_session`: 是否允许投递到当前会话之外
///
/// 返回:
/// - 已注册全部网格工具的注册表
fn registry_with_cross_session(
    paths: &SaiPaths,
    session: &SessionInfo,
    cross_session: bool,
) -> ToolRegistry {
    let state_dir = locate_session_dirs(paths, &session.id).unwrap().1;
    let mut registry = ToolRegistry::new();
    super::register(
        &mut registry,
        paths.clone(),
        state_dir.display().to_string(),
        session.id.clone(),
        cross_session,
    );
    registry
}
