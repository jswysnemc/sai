use super::super::app_state::WebAppState;
use super::super::error::{WebError, WebResult};
use crate::state::StateStore;
use axum::extract::{Path, Query, State};
use axum::routing::{get, patch, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use std::path::Path as FilePath;

mod permission_timeline;

#[derive(Serialize)]
struct SessionResponse {
    id: String,
    title: String,
    created_at: String,
    updated_at: String,
    active: bool,
}

#[derive(Serialize)]
struct WorkspaceSessionsResponse {
    workspace_id: String,
    workspace_name: String,
    workspace_path: String,
    active: bool,
    sessions: Vec<SessionResponse>,
}

#[derive(Deserialize)]
struct CreateSessionRequest {
    title: Option<String>,
    workspace_id: Option<String>,
}

#[derive(Deserialize)]
struct RenameSessionRequest {
    title: String,
}

#[derive(Deserialize)]
struct BulkDeleteSessionsRequest {
    ids: Vec<String>,
}

#[derive(Deserialize)]
struct ForkSessionRequest {
    turn_id: String,
    title: Option<String>,
}

#[derive(Deserialize)]
struct CompactSessionRequest {
    provider_id: Option<String>,
    model: Option<String>,
}

#[derive(Deserialize)]
struct RollbackSessionRequest {
    turn_id: String,
}

#[derive(Deserialize)]
struct HistoryQuery {
    limit: Option<usize>,
}

#[derive(Serialize)]
struct DeleteResponse {
    deleted: bool,
}

#[derive(Serialize)]
struct BulkDeleteResponse {
    deleted_ids: Vec<String>,
}

#[derive(Serialize)]
struct UndoSessionResponse {
    removed: usize,
    prompt: Option<String>,
    worktree_restored: bool,
}

#[derive(Serialize)]
struct RollbackSessionResponse {
    removed: usize,
    prompt: Option<String>,
}

/// 返回会话管理路由。
pub(super) fn routes() -> Router<WebAppState> {
    Router::new()
        .route("/api/sessions", get(list).post(create))
        .route("/api/sessions/tree", get(tree))
        .route("/api/sessions/bulk-delete", post(remove_many))
        .route("/api/sessions/:id", patch(rename).delete(remove))
        .route("/api/sessions/:id/switch", post(switch))
        .route("/api/sessions/:id/messages", get(messages))
        .route("/api/sessions/:id/timeline", get(timeline))
        .route("/api/sessions/:id/undo", post(undo))
        .route("/api/sessions/:id/rollback", post(rollback))
        .route("/api/sessions/:id/permission-audit", get(permission_audit))
        .route("/api/sessions/:id/compact", post(compact))
        .route("/api/sessions/:id/fork", post(fork))
}

/// 返回会话最近的权限审计事件。
///
/// 参数:
/// - `state`: Web 应用状态
/// - `id`: 会话标识
/// - `query`: 复用历史记录数量限制
///
/// 返回:
/// - JSON 审计事件列表
async fn permission_audit(
    State(state): State<WebAppState>,
    Path(id): Path<String>,
    Query(query): Query<HistoryQuery>,
) -> WebResult<Json<Vec<serde_json::Value>>> {
    let store = StateStore::for_session(&state.paths, &id).map_err(WebError::from)?;
    let path = store.state_dir().join("permission-audit.jsonl");
    if !path.exists() {
        return Ok(Json(Vec::new()));
    }
    let content = std::fs::read_to_string(path)
        .map_err(anyhow::Error::from)
        .map_err(WebError::from)?;
    let limit = query.limit.unwrap_or(100).clamp(1, 1000);
    let events = content
        .lines()
        .rev()
        .take(limit)
        .filter_map(|line| serde_json::from_str(line).ok())
        .collect::<Vec<_>>();
    Ok(Json(events))
}

/// 返回按工作区分组的全部会话。
///
/// 参数:
/// - `state`: Web 应用状态
///
/// 返回:
/// - 工作区及其会话树
async fn tree(State(state): State<WebAppState>) -> WebResult<Json<Vec<WorkspaceSessionsResponse>>> {
    let active_workspace = state.workspaces.active().map_err(WebError::from)?;
    let workspaces = state.workspaces.list().map_err(WebError::from)?;
    let mut result = Vec::with_capacity(workspaces.len());
    for workspace in workspaces {
        let path = FilePath::new(&workspace.path);
        let active_session_id = crate::state::active_session_id_for_workspace(&state.paths, path)
            .map_err(WebError::from)?;
        let sessions = crate::state::list_sessions_for_workspace(&state.paths, path)
            .map_err(WebError::from)?
            .into_iter()
            .map(|session| SessionResponse {
                active: workspace.id == active_workspace.id && session.id == active_session_id,
                id: session.id,
                title: session.title,
                created_at: session.created_at,
                updated_at: session.updated_at,
            })
            .collect();
        result.push(WorkspaceSessionsResponse {
            active: workspace.id == active_workspace.id,
            workspace_id: workspace.id,
            workspace_name: workspace.name,
            workspace_path: workspace.path,
            sessions,
        });
    }
    Ok(Json(result))
}

/// 手动压缩指定会话的旧轮次。
///
/// 参数:
/// - `state`: Web 应用状态
/// - `id`: 会话 ID
/// - `request`: 当前会话模型选择
///
/// 返回:
/// - 可订阅流式事件的运行信息
async fn compact(
    State(state): State<WebAppState>,
    Path(id): Path<String>,
    Json(request): Json<CompactSessionRequest>,
) -> WebResult<Json<crate::web::runs::ActiveRunInfo>> {
    let sessions = crate::state::list_sessions(&state.paths).map_err(WebError::from)?;
    if !sessions.iter().any(|session| session.id == id) {
        return Err(WebError::not_found(format!("session not found: {id}")));
    }
    let workspace = state.workspaces.active().map_err(WebError::from)?;
    let info = state
        .runs
        .start(
            workspace,
            crate::web::runs::StartRunRequest {
                kind: crate::web::runs::RunKind::Compaction,
                session_id: id,
                input: String::new(),
                agent_id: None,
                image_url: None,
                image_urls: Vec::new(),
                mode: None,
                provider_id: request.provider_id,
                model: request.model,
                thinking_level: None,
            },
        )
        .await
        .map_err(|error| WebError::conflict(error.to_string()))?;
    Ok(Json(info))
}

/// 列出当前工作区会话。
async fn list(State(state): State<WebAppState>) -> WebResult<Json<Vec<SessionResponse>>> {
    let active = crate::state::active_session(&state.paths).map_err(WebError::from)?;
    let sessions = crate::state::list_sessions(&state.paths).map_err(WebError::from)?;
    Ok(Json(
        sessions
            .into_iter()
            .map(|session| SessionResponse {
                active: session.id == active.id,
                id: session.id,
                title: session.title,
                created_at: session.created_at,
                updated_at: session.updated_at,
            })
            .collect(),
    ))
}

/// 从指定轮次分支出新会话，源会话不变。
async fn fork(
    State(state): State<WebAppState>,
    Path(id): Path<String>,
    Json(request): Json<ForkSessionRequest>,
) -> WebResult<Json<SessionResponse>> {
    let session = crate::state::fork_session_until_turn(
        &state.paths,
        &id,
        &request.turn_id,
        request.title.as_deref(),
    )
    .map_err(|error| WebError::bad_request(error.to_string()))?;
    Ok(Json(SessionResponse {
        id: session.id,
        title: session.title,
        created_at: session.created_at,
        updated_at: session.updated_at,
        active: true,
    }))
}

/// 创建并切换到新会话。
async fn create(
    State(state): State<WebAppState>,
    Json(request): Json<CreateSessionRequest>,
) -> WebResult<Json<SessionResponse>> {
    let workspace_active = request.workspace_id.is_none();
    let session = if let Some(workspace_id) = request.workspace_id.as_deref() {
        let workspace = state.workspaces.get(workspace_id).map_err(WebError::from)?;
        crate::state::create_session_for_workspace(
            &state.paths,
            FilePath::new(&workspace.path),
            request.title.as_deref(),
        )
    } else {
        crate::state::create_session(&state.paths, request.title.as_deref())
    }
    .map_err(WebError::from)?;
    Ok(Json(SessionResponse {
        id: session.id,
        title: session.title,
        created_at: session.created_at,
        updated_at: session.updated_at,
        active: workspace_active,
    }))
}

/// 切换当前会话。
async fn switch(
    State(state): State<WebAppState>,
    Path(id): Path<String>,
) -> WebResult<Json<SessionResponse>> {
    let session = crate::state::switch_session(&state.paths, &id)
        .map_err(|error| WebError::not_found(error.to_string()))?;
    Ok(Json(SessionResponse {
        id: session.id,
        title: session.title,
        created_at: session.created_at,
        updated_at: session.updated_at,
        active: true,
    }))
}

/// 重命名会话。
async fn rename(
    State(state): State<WebAppState>,
    Path(id): Path<String>,
    Json(request): Json<RenameSessionRequest>,
) -> WebResult<Json<SessionResponse>> {
    let session = crate::state::rename_session(&state.paths, &id, &request.title)
        .map_err(|error| WebError::bad_request(error.to_string()))?;
    let active = crate::state::active_session(&state.paths).map_err(WebError::from)?;
    Ok(Json(SessionResponse {
        active: session.id == active.id,
        id: session.id,
        title: session.title,
        created_at: session.created_at,
        updated_at: session.updated_at,
    }))
}

/// 删除会话。
async fn remove(
    State(state): State<WebAppState>,
    Path(id): Path<String>,
) -> WebResult<Json<DeleteResponse>> {
    reject_session_run(&state, &id).await?;
    let deleted = crate::state::delete_session(&state.paths, &id)
        .map_err(|error| WebError::bad_request(error.to_string()))?;
    Ok(Json(DeleteResponse { deleted }))
}

/// 批量删除会话。
///
/// 参数:
/// - `state`: Web 应用状态
/// - `request`: 待删除会话 ID 列表
///
/// 返回:
/// - 实际删除的会话 ID 列表
async fn remove_many(
    State(state): State<WebAppState>,
    Json(request): Json<BulkDeleteSessionsRequest>,
) -> WebResult<Json<BulkDeleteResponse>> {
    for id in &request.ids {
        reject_session_run(&state, id).await?;
    }
    if request.ids.is_empty() {
        return Err(WebError::bad_request("session ids cannot be empty"));
    }
    let deleted_ids = crate::state::delete_sessions(&state.paths, &request.ids)
        .map_err(|error| WebError::bad_request(error.to_string()))?;
    Ok(Json(BulkDeleteResponse { deleted_ids }))
}

/// 读取指定会话消息历史。
async fn messages(
    State(state): State<WebAppState>,
    Path(id): Path<String>,
    Query(query): Query<HistoryQuery>,
) -> WebResult<Json<Vec<crate::state::StoredConversationEntry>>> {
    let store = StateStore::for_session(&state.paths, &id)
        .map_err(|error| WebError::not_found(error.to_string()))?;
    let history = store
        .history(query.limit.unwrap_or(200).clamp(1, 2000))
        .map_err(WebError::from)?;
    Ok(Json(history))
}

/// 读取指定会话的结构化轮次与工具时间线。
///
/// 参数:
/// - `state`: Web 应用状态
/// - `id`: 会话 ID
/// - `query`: 轮次数量限制
///
/// 返回:
/// - 会话时间线
async fn timeline(
    State(state): State<WebAppState>,
    Path(id): Path<String>,
    Query(query): Query<HistoryQuery>,
) -> WebResult<Json<crate::state::SessionTimeline>> {
    let store = StateStore::for_session(&state.paths, &id)
        .map_err(|error| WebError::not_found(error.to_string()))?;
    let mut timeline = store
        .session_timeline_with_compaction(query.limit.unwrap_or(200).clamp(1, 2000))
        .map_err(WebError::from)?;
    permission_timeline::attach_permission_decisions(&store, &mut timeline.turns)
        .map_err(WebError::from)?;
    Ok(Json(timeline))
}

/// 撤销指定会话最后一轮及该轮造成的工作树修改。
///
/// 参数:
/// - `state`: Web 应用状态
/// - `id`: 会话标识
///
/// 返回:
/// - 撤销数量、恢复输入和工作树恢复状态
async fn undo(
    State(state): State<WebAppState>,
    Path(id): Path<String>,
) -> WebResult<Json<UndoSessionResponse>> {
    reject_session_run(&state, &id).await?;
    let store = StateStore::for_session(&state.paths, &id)
        .map_err(|error| WebError::not_found(error.to_string()))?;
    let outcome = store
        .undo_last_turn()
        .map_err(|error| WebError::conflict(error.to_string()))?;
    Ok(Json(UndoSessionResponse {
        removed: outcome.removed,
        prompt: outcome.prompt,
        worktree_restored: outcome.worktree_restored,
    }))
}

/// 仅回滚指定会话最后一轮上下文，不恢复工作树。
///
/// 参数:
/// - `state`: Web 应用状态
/// - `id`: 会话标识
/// - `request`: 前端准备重试的轮次标识
///
/// 返回:
/// - 删除数量和原用户输入
async fn rollback(
    State(state): State<WebAppState>,
    Path(id): Path<String>,
    Json(request): Json<RollbackSessionRequest>,
) -> WebResult<Json<RollbackSessionResponse>> {
    reject_session_run(&state, &id).await?;
    let store = StateStore::for_session(&state.paths, &id)
        .map_err(|error| WebError::not_found(error.to_string()))?;
    let outcome = store
        .rollback_last_turn_context(&request.turn_id)
        .map_err(|error| WebError::conflict(error.to_string()))?;
    Ok(Json(RollbackSessionResponse {
        removed: outcome.removed,
        prompt: outcome.prompt,
    }))
}

/// 活动运行期间仅禁止删除对应会话。
async fn reject_session_run(state: &WebAppState, session_id: &str) -> WebResult<()> {
    let workspace = state.workspaces.active().map_err(WebError::from)?;
    if state
        .runs
        .is_session_active(&workspace.id, session_id)
        .await
    {
        return Err(WebError::conflict(
            "stop the session run before deleting it",
        ));
    }
    Ok(())
}
