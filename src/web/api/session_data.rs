use super::super::app_state::WebAppState;
use super::super::error::{WebError, WebResult};
use crate::paths::SaiPaths;
use crate::state::{SessionInfo, StateStore};
use crate::tools::todo::TodoStore;
use anyhow::{bail, Context, Result};
use axum::extract::{Path, State};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Serialize;
use std::path::Path as FilePath;

/// 会话状态目录中的顶层数据项。
#[derive(Debug, Serialize)]
struct SessionDataItem {
    name: String,
    kind: String,
    bytes: u64,
    file_count: usize,
}

/// 单个会话的数据摘要。
#[derive(Debug, Serialize)]
struct SessionDataSummary {
    id: String,
    title: String,
    created_at: String,
    updated_at: String,
    active: bool,
    total_bytes: u64,
    file_count: usize,
    turn_count: Option<usize>,
    branch_points: Option<usize>,
    loaded_tool_count: Option<usize>,
    todo_count: Option<usize>,
    has_goal: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    state_error: Option<String>,
    items: Vec<SessionDataItem>,
}

#[derive(Debug, Serialize)]
struct ClearSessionDataResponse {
    cleared: bool,
}

#[derive(Default)]
struct StateMetrics {
    turn_count: Option<usize>,
    branch_points: Option<usize>,
    loaded_tool_count: Option<usize>,
    todo_count: Option<usize>,
    has_goal: Option<bool>,
    errors: Vec<String>,
}

#[derive(Default)]
struct FileStats {
    bytes: u64,
    file_count: usize,
}

/// 返回会话数据管理路由。
///
/// 参数:
/// - 无
///
/// 返回:
/// - 会话数据路由
pub(super) fn routes() -> Router<WebAppState> {
    Router::new()
        .route("/api/session-data", get(list))
        .route("/api/session-data/:id/clear", post(clear))
}

/// 列出当前工作区全部会话的数据摘要。
///
/// 参数:
/// - `state`: Web 应用状态
///
/// 返回:
/// - 会话数据摘要列表
async fn list(State(state): State<WebAppState>) -> WebResult<Json<Vec<SessionDataSummary>>> {
    let paths = state.paths.clone();
    let summaries = tokio::task::spawn_blocking(move || collect_session_data(&paths))
        .await
        .map_err(|error| WebError::from(anyhow::anyhow!(error)))?
        .map_err(WebError::from)?;
    Ok(Json(summaries))
}

/// 清理指定会话数据并保留会话元数据。
///
/// 参数:
/// - `state`: Web 应用状态
/// - `id`: 会话标识
///
/// 返回:
/// - 清理结果
async fn clear(
    State(state): State<WebAppState>,
    Path(id): Path<String>,
) -> WebResult<Json<ClearSessionDataResponse>> {
    let workspace_id = super::sessions::reject_session_run(&state, &id).await?;
    let owner_key = super::session_runtime::reject_running_subagents(&state.paths, &id)?;
    let paths = state.paths.clone();
    let clear_id = id.clone();
    tokio::task::spawn_blocking(move || clear_session_data(&paths, &clear_id))
        .await
        .map_err(|error| WebError::from(anyhow::anyhow!(error)))?
        .map_err(|error| WebError::bad_request(error.to_string()))?;
    super::session_runtime::clear_session_runtime_records(&owner_key, &id);
    state
        .runs
        .remove_session_history(&workspace_id, &id)
        .await
        .map_err(WebError::from)?;
    Ok(Json(ClearSessionDataResponse { cleared: true }))
}

/// 汇总当前工作区全部会话数据。
///
/// 参数:
/// - `paths`: Sai 路径集合
///
/// 返回:
/// - 会话数据摘要列表
fn collect_session_data(paths: &SaiPaths) -> Result<Vec<SessionDataSummary>> {
    let sessions = crate::state::list_sessions(paths)?;
    let active_id = crate::state::active_session(paths)?.id;
    sessions
        .into_iter()
        .map(|session| summarize_session_data(paths, session, &active_id))
        .collect()
}

/// 汇总单个会话的结构化状态与磁盘占用。
///
/// 参数:
/// - `paths`: Sai 路径集合
/// - `session`: 会话元数据
/// - `active_id`: 当前活动会话标识
///
/// 返回:
/// - 单会话数据摘要
fn summarize_session_data(
    paths: &SaiPaths,
    session: SessionInfo,
    active_id: &str,
) -> Result<SessionDataSummary> {
    let (_, state_dir) = crate::state::locate_session_dirs(paths, &session.id)?;
    let metrics = collect_state_metrics(paths, &session.id, &state_dir);
    let items = collect_top_level_items(&state_dir)?;
    let total_bytes = items.iter().map(|item| item.bytes).sum();
    let file_count = items.iter().map(|item| item.file_count).sum();
    Ok(SessionDataSummary {
        active: session.id == active_id,
        id: session.id,
        title: session.title,
        created_at: session.created_at,
        updated_at: session.updated_at,
        total_bytes,
        file_count,
        turn_count: metrics.turn_count,
        branch_points: metrics.branch_points,
        loaded_tool_count: metrics.loaded_tool_count,
        todo_count: metrics.todo_count,
        has_goal: metrics.has_goal,
        state_error: (!metrics.errors.is_empty()).then(|| metrics.errors.join("; ")),
        items,
    })
}

/// 读取会话数据库及辅助状态指标。
///
/// 参数:
/// - `paths`: Sai 路径集合
/// - `session_id`: 会话标识
/// - `state_dir`: 会话状态目录
///
/// 返回:
/// - 可用指标与逐项错误
fn collect_state_metrics(paths: &SaiPaths, session_id: &str, state_dir: &FilePath) -> StateMetrics {
    let mut metrics = StateMetrics::default();
    match StateStore::for_session(paths, session_id) {
        Ok(store) => {
            match store.session_tree() {
                Ok(tree) => {
                    metrics.turn_count = Some(tree.total_turns);
                    metrics.branch_points = Some(tree.branch_points);
                }
                Err(error) => metrics.errors.push(error.to_string()),
            }
            record_metric(
                store.load_loaded_tools().map(|items| items.len()),
                &mut metrics.loaded_tool_count,
                &mut metrics.errors,
            );
            record_metric(
                store.goal().map(|goal| goal.is_some()),
                &mut metrics.has_goal,
                &mut metrics.errors,
            );
        }
        Err(error) => {
            metrics.errors.push(error.to_string());
        }
    }
    // 1. 辅助文件独立统计，数据库损坏时仍尽量返回可用信息
    record_metric(
        TodoStore::new(state_dir.join("todos.json"))
            .list()
            .map(|items| items.len()),
        &mut metrics.todo_count,
        &mut metrics.errors,
    );
    metrics
}

/// 记录单项指标或错误。
///
/// 参数:
/// - `result`: 指标读取结果
/// - `target`: 指标输出位置
/// - `errors`: 错误列表
///
/// 返回:
/// - 无
fn record_metric<T>(result: Result<T>, target: &mut Option<T>, errors: &mut Vec<String>) {
    match result {
        Ok(value) => *target = Some(value),
        Err(error) => errors.push(error.to_string()),
    }
}

/// 收集状态目录顶层数据项。
///
/// 参数:
/// - `state_dir`: 会话状态目录
///
/// 返回:
/// - 按名称排序的数据项
fn collect_top_level_items(state_dir: &FilePath) -> Result<Vec<SessionDataItem>> {
    let mut items = Vec::new();
    for entry in std::fs::read_dir(state_dir)
        .with_context(|| format!("read session data directory {}", state_dir.display()))?
    {
        let entry = entry?;
        let path = entry.path();
        let metadata = std::fs::symlink_metadata(&path)?;
        let stats = collect_file_stats(&path)?;
        let kind = if metadata.is_dir() {
            "directory"
        } else if metadata.is_file() {
            "file"
        } else {
            "other"
        };
        items.push(SessionDataItem {
            name: entry.file_name().to_string_lossy().into_owned(),
            kind: kind.to_string(),
            bytes: stats.bytes,
            file_count: stats.file_count,
        });
    }
    items.sort_by(|left, right| left.name.cmp(&right.name));
    Ok(items)
}

/// 递归统计文件大小与文件数量，不跟随符号链接。
///
/// 参数:
/// - `path`: 文件或目录路径
///
/// 返回:
/// - 路径累计大小与文件数量
fn collect_file_stats(path: &FilePath) -> Result<FileStats> {
    let metadata = std::fs::symlink_metadata(path)?;
    if !metadata.is_dir() {
        return Ok(FileStats {
            bytes: metadata.len(),
            file_count: 1,
        });
    }
    let mut total = FileStats::default();
    for entry in std::fs::read_dir(path)? {
        let stats = collect_file_stats(&entry?.path())?;
        total.bytes = total.bytes.saturating_add(stats.bytes);
        total.file_count = total.file_count.saturating_add(stats.file_count);
    }
    Ok(total)
}

/// 清空指定会话状态目录并重新初始化最小状态文件。
///
/// 参数:
/// - `paths`: Sai 路径集合
/// - `session_id`: 会话标识
///
/// 返回:
/// - 清理结果
fn clear_session_data(paths: &SaiPaths, session_id: &str) -> Result<()> {
    if !crate::state::list_sessions(paths)?
        .iter()
        .any(|session| session.id == session_id)
    {
        bail!("session not found in current workspace: {session_id}");
    }
    let (_, state_dir) = crate::state::locate_session_dirs(paths, session_id)?;
    // 1. 删除整个状态目录，避免辅助文件残留
    if state_dir.exists() {
        std::fs::remove_dir_all(&state_dir)
            .with_context(|| format!("remove session data {}", state_dir.display()))?;
    }
    std::fs::create_dir_all(&state_dir)?;
    // 2. 重建数据库与基础文件，会话索引和标题保持不变
    StateStore::for_session(paths, session_id)?.init_files()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::paths::SaiPaths;
    use crate::state::StateStore;
    use std::path::Path;

    /// 创建隔离的会话数据测试路径。
    ///
    /// 参数:
    /// - `root`: 临时目录
    ///
    /// 返回:
    /// - 测试用 Sai 路径
    fn test_paths(root: &Path) -> SaiPaths {
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

    /// 验证统计包含结构化状态与顶层数据项。
    #[tokio::test]
    async fn session_data_summary_reports_state_contents() {
        let temp = tempfile::tempdir().unwrap();
        let paths = test_paths(temp.path());
        let workspace = temp.path().join("workspace");
        std::fs::create_dir_all(&workspace).unwrap();

        crate::runtime_cwd::scope(workspace, async {
            let session = crate::state::create_session(&paths, Some("managed")).unwrap();
            let store = StateStore::for_session(&paths, &session.id).unwrap();
            store.start_turn("turn-1", "question").unwrap();
            store.complete_turn("turn-1", "answer", None).unwrap();
            std::fs::write(store.state_dir().join("todos.json"), "[]").unwrap();
            drop(store);

            let summaries = collect_session_data(&paths).unwrap();
            let summary = summaries.iter().find(|item| item.id == session.id).unwrap();

            assert_eq!(summary.turn_count, Some(1));
            assert!(summary.total_bytes > 0);
            assert!(summary
                .items
                .iter()
                .any(|item| item.name == "conversation.db"));
            assert!(summary.items.iter().any(|item| item.name == "todos.json"));
        })
        .await;
    }

    /// 验证清理只删除会话内容，并保留会话索引与标题。
    #[tokio::test]
    async fn clear_session_data_preserves_session_metadata() {
        let temp = tempfile::tempdir().unwrap();
        let paths = test_paths(temp.path());
        let workspace = temp.path().join("workspace");
        std::fs::create_dir_all(&workspace).unwrap();

        crate::runtime_cwd::scope(workspace, async {
            let session = crate::state::create_session(&paths, Some("keep title")).unwrap();
            let store = StateStore::for_session(&paths, &session.id).unwrap();
            store.start_turn("turn-1", "question").unwrap();
            store.complete_turn("turn-1", "answer", None).unwrap();
            let marker = store.state_dir().join("nested/marker.txt");
            std::fs::create_dir_all(marker.parent().unwrap()).unwrap();
            std::fs::write(&marker, "remove me").unwrap();
            drop(store);

            clear_session_data(&paths, &session.id).unwrap();

            let metadata = crate::state::list_sessions(&paths)
                .unwrap()
                .into_iter()
                .find(|item| item.id == session.id)
                .unwrap();
            assert_eq!(metadata.title, "keep title");
            assert!(!marker.exists());
            let reopened = StateStore::for_session(&paths, &session.id).unwrap();
            assert!(reopened.load_all_turns().unwrap().is_empty());
            assert!(reopened.state_dir().join("usage.json").is_file());
            assert!(reopened.state_dir().join("profile.md").is_file());
        })
        .await;
    }
}
