use super::super::app_state::WebAppState;
use super::super::error::{WebError, WebResult};
use crate::paths::SaiPaths;
use crate::state::{SessionInfo, StateStore};
use anyhow::{bail, Context, Result};
use axum::extract::{Path, State};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use std::path::{Path as FilePath, PathBuf};

#[path = "session_data_todos.rs"]
mod todos;

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
    workspace_id: String,
    workspace_name: String,
    workspace_path: String,
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

#[derive(Debug, Deserialize)]
struct ClearSessionDataRequest {
    sessions: Vec<SessionDataSelection>,
}

#[derive(Debug, Deserialize, Clone)]
struct SessionDataSelection {
    workspace_id: String,
    session_id: String,
}

#[derive(Debug, Serialize)]
struct ClearSessionDataResponse {
    cleared: bool,
    cleared_ids: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct DeleteSessionDataRequest {
    sessions: Vec<SessionDataSelection>,
}

#[derive(Debug, Serialize)]
struct DeleteSessionDataResponse {
    deleted_ids: Vec<String>,
    /// 索引中不存在、因而未被删除的会话，交给前端提示而不是静默吞掉
    missing_ids: Vec<String>,
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
        .route("/api/session-data/clear", post(clear_many))
        .route("/api/session-data/delete", post(delete_many))
        .route("/api/session-data/:id/clear", post(clear))
}

/// 列出所有已登记工作区的会话数据摘要。
///
/// 参数:
/// - `state`: Web 应用状态
///
/// 返回:
/// - 会话数据摘要列表
async fn list(State(state): State<WebAppState>) -> WebResult<Json<Vec<SessionDataSummary>>> {
    let paths = state.paths.clone();
    let active_workspace = state.workspaces.active().map_err(WebError::from)?;
    let workspaces = state.workspaces.list().map_err(WebError::from)?;
    let mut summaries = tokio::task::spawn_blocking(move || {
        collect_session_data(&paths, &workspaces, &active_workspace.id)
    })
    .await
    .map_err(|error| WebError::from(anyhow::anyhow!(error)))?
    .map_err(WebError::from)?;
    todos::fill_counts(&state.paths, &mut summaries)
        .await
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
    // 1. 必须按会话所属工作区定位：locate_session_dirs 会先扫服务端当前工作区，
    //    对各工作区都存在的 default 等同名会话会清掉错误目标
    let workspaces = state.workspaces.list().map_err(WebError::from)?;
    let workspace_path = workspaces
        .iter()
        .find(|workspace| workspace.id == workspace_id)
        .map(|workspace| PathBuf::from(&workspace.path))
        .ok_or_else(|| WebError::not_found(format!("workspace not found: {workspace_id}")))?;
    let owner_key = super::session_runtime::reject_running_subagents_for_workspace(
        &state.paths,
        &workspace_path,
        &id,
    )?;
    let paths = state.paths.clone();
    let clear_id = id.clone();
    tokio::task::spawn_blocking(move || {
        clear_session_data_for_workspace(&paths, &workspace_path, &clear_id)
    })
    .await
    .map_err(|error| WebError::from(anyhow::anyhow!(error)))?
    .map_err(|error| WebError::bad_request(error.to_string()))?;
    super::session_runtime::clear_session_runtime_records(&owner_key, &id);
    state
        .runs
        .remove_session_history(&workspace_id, &id)
        .await
        .map_err(WebError::from)?;
    Ok(Json(ClearSessionDataResponse {
        cleared: true,
        cleared_ids: vec![id],
    }))
}

/// 清空多个工作区中的会话数据并保留会话条目。
///
/// 参数:
/// - `state`: Web 应用状态
/// - `request`: 要清空的工作区和会话标识
///
/// 返回:
/// - 已清空的会话标识
async fn clear_many(
    State(state): State<WebAppState>,
    Json(request): Json<ClearSessionDataRequest>,
) -> WebResult<Json<ClearSessionDataResponse>> {
    clear_selected_sessions(state, request.sessions).await
}

/// 校验并执行一批跨工作区会话数据清理。
///
/// 参数:
/// - `state`: Web 应用状态
/// - `selections`: 工作区与会话选择
///
/// 返回:
/// - 清理结果
async fn clear_selected_sessions(
    state: WebAppState,
    selections: Vec<SessionDataSelection>,
) -> WebResult<Json<ClearSessionDataResponse>> {
    let selections = dedupe_selections(selections);
    if selections.is_empty() {
        return Err(WebError::bad_request(
            "at least one session must be selected",
        ));
    }
    let workspaces = state.workspaces.list().map_err(WebError::from)?;
    let workspace_paths = selections
        .iter()
        .map(|selection| {
            let workspace = workspaces
                .iter()
                .find(|workspace| workspace.id == selection.workspace_id)
                .ok_or_else(|| {
                    WebError::not_found(format!("workspace not found: {}", selection.workspace_id))
                })?;
            Ok((selection.clone(), PathBuf::from(&workspace.path)))
        })
        .collect::<WebResult<Vec<_>>>()?;

    let mut owners = Vec::with_capacity(selections.len());
    for (selection, workspace_path) in &workspace_paths {
        // 1. 按工作区路径定位，避免不同工作区的 default 等同名会话互相串线
        let owner_key = super::session_runtime::reject_running_subagents_for_workspace(
            &state.paths,
            workspace_path,
            &selection.session_id,
        )?;
        // 2. 运行状态同样使用请求中的工作区作用域检查
        if state
            .runs
            .is_session_active(&selection.workspace_id, &selection.session_id)
            .await
        {
            return Err(WebError::conflict(
                "stop the session run before modifying it",
            ));
        }
        owners.push((
            selection.session_id.clone(),
            selection.workspace_id.clone(),
            owner_key,
        ));
    }

    let paths = state.paths.clone();
    let clear_inputs = workspace_paths.clone();
    tokio::task::spawn_blocking(move || {
        for (selection, workspace_path) in &clear_inputs {
            clear_session_data_for_workspace(&paths, workspace_path, &selection.session_id)?;
        }
        Ok::<(), anyhow::Error>(())
    })
    .await
    .map_err(|error| WebError::from(anyhow::anyhow!(error)))?
    .map_err(|error| WebError::bad_request(error.to_string()))?;

    for (session_id, workspace_id, owner_key) in owners {
        super::session_runtime::clear_session_runtime_records(&owner_key, &session_id);
        state
            .runs
            .remove_session_history(&workspace_id, &session_id)
            .await
            .map_err(WebError::from)?;
    }
    Ok(Json(ClearSessionDataResponse {
        cleared: true,
        cleared_ids: selections.into_iter().map(|item| item.session_id).collect(),
    }))
}

/// 删除多个工作区中的会话及其全部数据。
///
/// 会话按工作区分作用域存放，各工作区可以有同名的 default 等会话，因此删除
/// 必须带上所属工作区。此前前端复用了只按会话 ID 删除的接口，落到服务端当前
/// 工作区的作用域里查找，别的工作区的会话在索引中找不到，请求返回成功却一个
/// 都没删掉。
///
/// 参数:
/// - `state`: Web 应用状态
/// - `request`: 待删除的工作区与会话选择
///
/// 返回:
/// - 实际删除与未找到的会话标识
async fn delete_many(
    State(state): State<WebAppState>,
    Json(request): Json<DeleteSessionDataRequest>,
) -> WebResult<Json<DeleteSessionDataResponse>> {
    let selections = dedupe_selections(request.sessions);
    if selections.is_empty() {
        return Err(WebError::bad_request(
            "at least one session must be selected",
        ));
    }

    // 1. 把每个选择解析到所属工作区的真实路径
    let workspaces = state.workspaces.list().map_err(WebError::from)?;
    let resolved = selections
        .iter()
        .map(|selection| {
            let workspace = workspaces
                .iter()
                .find(|workspace| workspace.id == selection.workspace_id)
                .ok_or_else(|| {
                    WebError::not_found(format!("workspace not found: {}", selection.workspace_id))
                })?;
            Ok((selection.clone(), PathBuf::from(&workspace.path)))
        })
        .collect::<WebResult<Vec<_>>>()?;

    // 2. 运行中的会话与子智能体一律拒绝，避免删掉正在写入的目录
    let mut owners = Vec::with_capacity(resolved.len());
    for (selection, workspace_path) in &resolved {
        let owner_key = super::session_runtime::reject_running_subagents_for_workspace(
            &state.paths,
            workspace_path,
            &selection.session_id,
        )?;
        if state
            .runs
            .is_session_active(&selection.workspace_id, &selection.session_id)
            .await
        {
            return Err(WebError::conflict(
                "stop the session run before modifying it",
            ));
        }
        owners.push((
            selection.session_id.clone(),
            selection.workspace_id.clone(),
            owner_key,
        ));
    }

    // 3. 按工作区归组，同一工作区的多个会话共用一次索引写入
    let mut grouped: Vec<(PathBuf, Vec<String>)> = Vec::new();
    for (selection, workspace_path) in &resolved {
        match grouped.iter_mut().find(|(path, _)| path == workspace_path) {
            Some((_, ids)) => ids.push(selection.session_id.clone()),
            None => grouped.push((workspace_path.clone(), vec![selection.session_id.clone()])),
        }
    }

    let paths = state.paths.clone();
    let deleted_ids = tokio::task::spawn_blocking(move || {
        let mut deleted = Vec::new();
        for (workspace_path, session_ids) in &grouped {
            deleted.extend(crate::state::delete_sessions_for_workspace(
                &paths,
                workspace_path,
                session_ids,
            )?);
        }
        Ok::<Vec<String>, anyhow::Error>(deleted)
    })
    .await
    .map_err(|error| WebError::from(anyhow::anyhow!(error)))?
    .map_err(|error| WebError::bad_request(error.to_string()))?;

    // 4. 只为真正删掉的会话清理运行时痕迹
    for (session_id, workspace_id, owner_key) in &owners {
        if !deleted_ids.contains(session_id) {
            continue;
        }
        super::session_runtime::clear_session_runtime_records(owner_key, session_id);
        state
            .runs
            .remove_session_history(workspace_id, session_id)
            .await
            .map_err(WebError::from)?;
    }

    let missing_ids = selections
        .iter()
        .map(|selection| selection.session_id.clone())
        .filter(|id| !deleted_ids.contains(id))
        .collect::<Vec<_>>();
    Ok(Json(DeleteSessionDataResponse {
        deleted_ids,
        missing_ids,
    }))
}

/// 去重会话选择，保持用户首次选择的顺序。
///
/// 参数:
/// - `selections`: 原始选择
///
/// 返回:
/// - 去重后的选择
fn dedupe_selections(selections: Vec<SessionDataSelection>) -> Vec<SessionDataSelection> {
    let mut seen = std::collections::HashSet::new();
    selections
        .into_iter()
        .filter(|item| {
            !item.workspace_id.trim().is_empty()
                && !item.session_id.trim().is_empty()
                && seen.insert((item.workspace_id.clone(), item.session_id.clone()))
        })
        .collect()
}

/// 汇总当前工作区全部会话数据。
///
/// 参数:
/// - `paths`: Sai 路径集合
///
/// 返回:
/// - 会话数据摘要列表
fn collect_session_data(
    paths: &SaiPaths,
    workspaces: &[super::super::workspaces::WorkspaceInfo],
    active_workspace_id: &str,
) -> Result<Vec<SessionDataSummary>> {
    let mut summaries = Vec::new();
    for workspace in workspaces {
        let workspace_path = FilePath::new(&workspace.path);
        let active_id = crate::state::active_session_id_for_workspace(paths, workspace_path)?;
        let sessions = crate::state::list_sessions_for_workspace(paths, workspace_path)?;
        for session in sessions {
            summaries.push(summarize_session_data(
                paths,
                workspace,
                workspace_path,
                session,
                active_workspace_id == workspace.id,
                &active_id,
            )?);
        }
    }
    summaries.sort_by(|left, right| {
        left.workspace_name
            .cmp(&right.workspace_name)
            .then_with(|| right.updated_at.cmp(&left.updated_at))
    });
    Ok(summaries)
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
    workspace: &super::super::workspaces::WorkspaceInfo,
    workspace_path: &FilePath,
    session: SessionInfo,
    workspace_active: bool,
    active_id: &str,
) -> Result<SessionDataSummary> {
    let (_, state_dir) =
        crate::state::state_dir_for_workspace_session(paths, workspace_path, &session.id)?;
    let metrics = collect_state_metrics(paths, workspace_path, &session.id);
    let items = collect_top_level_items(&state_dir)?;
    let total_bytes = items.iter().map(|item| item.bytes).sum();
    let file_count = items.iter().map(|item| item.file_count).sum();
    Ok(SessionDataSummary {
        workspace_id: workspace.id.clone(),
        workspace_name: workspace.name.clone(),
        workspace_path: workspace.path.clone(),
        active: workspace_active && session.id == active_id,
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
/// - `workspace_path`: 工作区目录
/// - `session_id`: 会话标识
///
/// 返回:
/// - 可用指标与逐项错误
fn collect_state_metrics(
    paths: &SaiPaths,
    workspace_path: &FilePath,
    session_id: &str,
) -> StateMetrics {
    let mut metrics = StateMetrics::default();
    match StateStore::for_workspace_session(paths, workspace_path, session_id) {
        Ok(store) => {
            match store.session_tree_counts() {
                Ok((turns, branches)) => {
                    metrics.turn_count = Some(turns);
                    metrics.branch_points = Some(branches);
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

/// 清空指定工作区中的会话状态目录并重新初始化最小状态文件。
///
/// 参数:
/// - `paths`: Sai 路径集合
/// - `workspace_path`: 会话所属工作区目录
/// - `session_id`: 会话标识
///
/// 返回:
/// - 清理结果
fn clear_session_data_for_workspace(
    paths: &SaiPaths,
    workspace_path: &FilePath,
    session_id: &str,
) -> Result<()> {
    if !crate::state::list_sessions_for_workspace(paths, workspace_path)?
        .iter()
        .any(|session| session.id == session_id)
    {
        bail!("session not found in workspace: {session_id}");
    }
    let (_, state_dir) =
        crate::state::state_dir_for_workspace_session(paths, workspace_path, session_id)?;
    clear_state_directory(paths, session_id, &state_dir, Some(workspace_path))
}

/// 删除会话状态目录并使用对应工作区重新建立基础文件。
///
/// 参数:
/// - `paths`: Sai 路径集合
/// - `session_id`: 会话标识
/// - `state_dir`: 待清理状态目录
/// - `workspace_path`: 可选所属工作区，缺省按全局会话定位
///
/// 返回:
/// - 清理结果
fn clear_state_directory(
    paths: &SaiPaths,
    session_id: &str,
    state_dir: &FilePath,
    workspace_path: Option<&FilePath>,
) -> Result<()> {
    // 1. 【会话数据】【清理插件记录】会话外的公共存储按完整目录清理，保留其他作用域
    crate::plugins::clear_session_storage(&paths.state_dir, &state_dir.to_string_lossy())?;
    // 2. 删除整个状态目录，避免辅助文件残留
    if state_dir.exists() {
        std::fs::remove_dir_all(&state_dir)
            .with_context(|| format!("remove session data {}", state_dir.display()))?;
    }
    std::fs::create_dir_all(&state_dir)?;
    // 3. 重建数据库与基础文件，会话索引和标题保持不变
    match workspace_path {
        Some(path) => StateStore::for_workspace_session(paths, path, session_id)?.init_files(),
        None => StateStore::for_session(paths, session_id)?.init_files(),
    }
}

#[cfg(test)]
#[path = "session_data_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "session_data_todo_tests.rs"]
mod todo_tests;
