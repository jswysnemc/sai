use super::super::app_state::WebAppState;
use super::super::error::{WebError, WebResult};
use crate::paths::SaiPaths;
use crate::state::{SessionInfo, StateStore};
use crate::tools::todo::TodoStore;
use anyhow::{bail, Context, Result};
use axum::extract::{Path, State};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use std::path::{Path as FilePath, PathBuf};

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
    let summaries = tokio::task::spawn_blocking(move || {
        collect_session_data(&paths, &workspaces, &active_workspace.id)
    })
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
    let metrics = collect_state_metrics(paths, workspace_path, &session.id, &state_dir);
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
/// - `state_dir`: 会话状态目录
///
/// 返回:
/// - 可用指标与逐项错误
fn collect_state_metrics(
    paths: &SaiPaths,
    workspace_path: &FilePath,
    session_id: &str,
    state_dir: &FilePath,
) -> StateMetrics {
    let mut metrics = StateMetrics::default();
    match StateStore::for_workspace_session(paths, workspace_path, session_id) {
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
    // 1. 删除整个状态目录，避免辅助文件残留
    if state_dir.exists() {
        std::fs::remove_dir_all(&state_dir)
            .with_context(|| format!("remove session data {}", state_dir.display()))?;
    }
    std::fs::create_dir_all(&state_dir)?;
    // 2. 重建数据库与基础文件，会话索引和标题保持不变
    match workspace_path {
        Some(path) => StateStore::for_workspace_session(paths, path, session_id)?.init_files(),
        None => StateStore::for_session(paths, session_id)?.init_files(),
    }
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

        crate::runtime_cwd::scope(workspace.clone(), async {
            let session = crate::state::create_session(&paths, Some("managed")).unwrap();
            let store = StateStore::for_session(&paths, &session.id).unwrap();
            store.start_turn("turn-1", "question").unwrap();
            store.complete_turn("turn-1", "answer", None).unwrap();
            std::fs::write(store.state_dir().join("todos.json"), "[]").unwrap();
            drop(store);

            let workspace_id = crate::state::workspace_id_for_path(&workspace);
            let workspace_info = crate::web::workspaces::WorkspaceInfo {
                id: workspace_id.clone(),
                name: "managed".to_string(),
                path: workspace.display().to_string(),
                last_opened_at: String::new(),
            };
            let summaries = collect_session_data(&paths, &[workspace_info], &workspace_id).unwrap();
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
        let clear_workspace = workspace.clone();

        crate::runtime_cwd::scope(workspace, async move {
            let session = crate::state::create_session(&paths, Some("keep title")).unwrap();
            let store = StateStore::for_session(&paths, &session.id).unwrap();
            store.start_turn("turn-1", "question").unwrap();
            store.complete_turn("turn-1", "answer", None).unwrap();
            let marker = store.state_dir().join("nested/marker.txt");
            std::fs::create_dir_all(marker.parent().unwrap()).unwrap();
            std::fs::write(&marker, "remove me").unwrap();
            drop(store);

            clear_session_data_for_workspace(&paths, &clear_workspace, &session.id).unwrap();

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

    /// 会话数据面板横跨所有工作区，删除必须落到会话真正所属的作用域。
    ///
    /// 回归此前的缺陷：删除只按会话 ID 在服务端当前工作区的索引里查找，
    /// 别的工作区的会话找不到就静默返回成功，界面刷新后会话原样还在。
    #[tokio::test]
    async fn deletes_sessions_from_a_non_current_workspace() {
        let temp = tempfile::tempdir().unwrap();
        let paths = test_paths(temp.path());
        let workspace_a = temp.path().join("workspace-a");
        let workspace_b = temp.path().join("workspace-b");
        std::fs::create_dir_all(&workspace_a).unwrap();
        std::fs::create_dir_all(&workspace_b).unwrap();

        // 1. 在工作区 B 里建一个会话
        let session_b = crate::runtime_cwd::scope(workspace_b.clone(), async {
            crate::state::create_session(&paths, Some("b session")).unwrap()
        })
        .await;

        // 2. 把当前工作区切到 A，此时 B 的会话不在当前作用域的索引里
        let target_b = session_b.id.clone();
        let deleted = crate::runtime_cwd::scope(workspace_a.clone(), async {
            let stale = crate::state::delete_sessions(&paths, &[target_b.clone()]).unwrap();
            assert!(
                stale.is_empty(),
                "按当前工作区删除跨工作区会话本就删不掉，这里固定住该前提"
            );
            crate::state::delete_sessions_for_workspace(&paths, &workspace_b, &[target_b.clone()])
                .unwrap()
        })
        .await;

        // 3. 指定工作区后必须真的删掉
        assert_eq!(deleted, vec![session_b.id.clone()]);
        let remaining = crate::state::list_sessions_for_workspace(&paths, &workspace_b).unwrap();
        assert!(
            !remaining.iter().any(|item| item.id == session_b.id),
            "会话应已从所属工作区的索引中移除"
        );
    }
}
