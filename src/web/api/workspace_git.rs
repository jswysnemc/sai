use super::super::app_state::WebAppState;
use super::super::error::{WebError, WebResult};
use super::super::workspace;
use axum::extract::{Query, State};
use axum::routing::get;
use axum::{Json, Router};
use serde::Deserialize;
use std::path::{Path, PathBuf};

#[derive(Deserialize)]
struct GitRepositoryQuery {
    repo_root: Option<String>,
}

#[derive(Deserialize)]
struct GitActionRequest {
    action: String,
    #[serde(default)]
    paths: Vec<String>,
    message: Option<String>,
    repo_root: Option<String>,
}

#[derive(Deserialize)]
struct GitStatusesRequest {
    #[serde(default)]
    repo_roots: Vec<String>,
}

#[derive(Deserialize)]
struct GitCloneRequest {
    remote_url: String,
    parent: String,
    directory: Option<String>,
}

#[derive(Deserialize)]
struct GitOpRequest {
    action: String,
    repo_root: Option<String>,
    path: Option<String>,
    old_path: Option<String>,
    message: Option<String>,
    remote_url: Option<String>,
    branch: Option<String>,
    branch_kind: Option<String>,
    new_branch: Option<String>,
    start_point: Option<String>,
    post_action: Option<String>,
    patch: Option<String>,
    commit: Option<String>,
    reset_mode: Option<String>,
    stash_ref: Option<String>,
    tag: Option<String>,
    remote_name: Option<String>,
    worktree_path: Option<String>,
    #[serde(default)]
    include_untracked: bool,
    resolution: Option<String>,
    content: Option<String>,
    #[serde(default)]
    all: bool,
    #[serde(default)]
    amend: bool,
    #[serde(default)]
    signoff: bool,
    #[serde(default)]
    force: bool,
}

#[derive(Deserialize)]
struct GitDiffQuery {
    repo_root: Option<String>,
    mode: Option<String>,
    path: Option<String>,
}

#[derive(Deserialize)]
struct GitLogQuery {
    repo_root: Option<String>,
    limit: Option<usize>,
    skip: Option<usize>,
}

#[derive(Deserialize)]
struct GitCommitQuery {
    repo_root: Option<String>,
    commit: String,
    path: Option<String>,
}

#[derive(Deserialize)]
struct GitConflictQuery {
    repo_root: Option<String>,
    path: String,
}

/// 返回工作区 Git 路由。
///
/// 返回:
/// - Git 状态、资源、历史和操作路由
pub(super) fn routes() -> Router<WebAppState> {
    Router::new()
        .route("/api/workspace/git", axum::routing::post(git_action))
        .route("/api/workspace/git/repositories", get(git_repositories))
        .route("/api/workspace/git/status", get(git_status))
        .route("/api/workspace/git/clone", axum::routing::post(git_clone))
        .route(
            "/api/workspace/git/statuses",
            axum::routing::post(git_statuses),
        )
        .route("/api/workspace/git/branches", get(git_branches))
        .route("/api/workspace/git/log", get(git_log))
        .route("/api/workspace/git/resources", get(git_resources))
        .route("/api/workspace/git/conflict", get(git_conflict))
        .route("/api/workspace/git/commit", get(git_commit_details))
        .route("/api/workspace/git/commit-diff", get(git_commit_diff))
        .route("/api/workspace/git/diff", get(git_review_diff))
        .route("/api/workspace/git/op", axum::routing::post(git_op))
}

/// 批量读取当前工作区多个仓库的完整状态。
///
/// 参数:
/// - `state`: Web 应用状态
/// - `request`: 待读取仓库根目录列表
///
/// 返回:
/// - 并发上限受控的多仓库状态
async fn git_statuses(
    State(state): State<WebAppState>,
    Json(request): Json<GitStatusesRequest>,
) -> WebResult<Json<workspace::GitRepositoryStatusesResponse>> {
    let active = state.workspaces.active().map_err(WebError::from)?;
    let statuses = workspace::git_repository_statuses(Path::new(&active.path), &request.repo_roots)
        .await
        .map_err(|error| WebError::bad_request(error.to_string()))?;
    Ok(Json(statuses))
}

/// 将远端仓库克隆到服务端允许的目标目录。
///
/// 参数:
/// - `request`: 远端地址、父目录和可选目录名
///
/// 返回:
/// - Git 输出与克隆后仓库状态
async fn git_clone(
    Json(request): Json<GitCloneRequest>,
) -> WebResult<Json<workspace::GitOperationResponse>> {
    // 1. 目标父目录必须位于服务端工作区允许范围
    let parent = super::super::workspaces::validate_browsable_directory(&request.parent)
        .map_err(|error| WebError::bad_request(error.to_string()))?;
    // 2. 克隆命令使用系统 Git，并返回真实标准输出和错误
    let result = workspace::git_clone(&parent, &request.remote_url, request.directory.as_deref())
        .await
        .map_err(|error| WebError::bad_request(error.to_string()))?;
    Ok(Json(result))
}

/// 执行兼容版 Git 暂存、取消暂存、撤销或提交操作。
///
/// 参数:
/// - `state`: Web 应用状态
/// - `request`: Git 操作参数
///
/// 返回:
/// - 刷新后的兼容版 Git Diff
async fn git_action(
    State(state): State<WebAppState>,
    Json(request): Json<GitActionRequest>,
) -> WebResult<Json<workspace::GitDiff>> {
    let root = request_repository_root(&state, request.repo_root.as_deref()).await?;
    let git = workspace::apply_git_action(
        &root,
        &request.action,
        &request.paths,
        request.message.as_deref(),
    )
    .await
    .map_err(|error| WebError::bad_request(error.to_string()))?;
    Ok(Json(git))
}

/// 发现当前工作区的 Git 仓库与关联 worktree。
///
/// 参数:
/// - `state`: Web 应用状态
///
/// 返回:
/// - 仓库摘要列表
async fn git_repositories(
    State(state): State<WebAppState>,
) -> WebResult<Json<workspace::GitRepositoriesResponse>> {
    let active = state.workspaces.active().map_err(WebError::from)?;
    let repositories = workspace::git_repositories(Path::new(&active.path))
        .await
        .map_err(WebError::from)?;
    Ok(Json(repositories))
}

/// 读取选中仓库 Git 状态。
///
/// 参数:
/// - `state`: Web 应用状态
/// - `query`: 可选仓库根目录
///
/// 返回:
/// - 仓库完整状态
async fn git_status(
    State(state): State<WebAppState>,
    Query(query): Query<GitRepositoryQuery>,
) -> WebResult<Json<workspace::GitRepositoryState>> {
    let root = request_repository_root(&state, query.repo_root.as_deref()).await?;
    let status = workspace::git_status(&root).await.map_err(WebError::from)?;
    Ok(Json(status))
}

/// 读取选中仓库分支列表。
///
/// 参数:
/// - `state`: Web 应用状态
/// - `query`: 可选仓库根目录
///
/// 返回:
/// - 仓库分支列表
async fn git_branches(
    State(state): State<WebAppState>,
    Query(query): Query<GitRepositoryQuery>,
) -> WebResult<Json<workspace::GitBranchesResponse>> {
    let root = request_repository_root(&state, query.repo_root.as_deref()).await?;
    let branches = workspace::git_branches(&root)
        .await
        .map_err(WebError::from)?;
    Ok(Json(branches))
}

/// 读取选中仓库提交历史。
///
/// 参数:
/// - `state`: Web 应用状态
/// - `query`: 仓库根目录与分页参数
///
/// 返回:
/// - 提交图历史数据
async fn git_log(
    State(state): State<WebAppState>,
    Query(query): Query<GitLogQuery>,
) -> WebResult<Json<workspace::GitLogResponse>> {
    let root = request_repository_root(&state, query.repo_root.as_deref()).await?;
    let log = workspace::git_log(&root, query.limit, query.skip)
        .await
        .map_err(WebError::from)?;
    Ok(Json(log))
}

/// 读取选中仓库的 stash、标签和远端资源。
///
/// 参数:
/// - `state`: Web 应用状态
/// - `query`: 可选仓库根目录
///
/// 返回:
/// - 仓库资源列表
async fn git_resources(
    State(state): State<WebAppState>,
    Query(query): Query<GitRepositoryQuery>,
) -> WebResult<Json<workspace::GitRepositoryResources>> {
    let root = request_repository_root(&state, query.repo_root.as_deref()).await?;
    let resources = workspace::git_resources(&root)
        .await
        .map_err(WebError::from)?;
    Ok(Json(resources))
}

/// 读取选中仓库单个冲突文件内容。
///
/// 参数:
/// - `state`: Web 应用状态
/// - `query`: 仓库根目录与冲突文件路径
///
/// 返回:
/// - Merge Editor 冲突内容
async fn git_conflict(
    State(state): State<WebAppState>,
    Query(query): Query<GitConflictQuery>,
) -> WebResult<Json<workspace::GitConflictContent>> {
    let root = request_repository_root(&state, query.repo_root.as_deref()).await?;
    let conflict = workspace::git_conflict(&root, &query.path)
        .await
        .map_err(|error| WebError::bad_request(error.to_string()))?;
    Ok(Json(conflict))
}

/// 读取选中仓库提交详情。
///
/// 参数:
/// - `state`: Web 应用状态
/// - `query`: 仓库根目录与提交标识
///
/// 返回:
/// - 提交元数据和文件列表
async fn git_commit_details(
    State(state): State<WebAppState>,
    Query(query): Query<GitCommitQuery>,
) -> WebResult<Json<workspace::GitCommitDetailsResponse>> {
    let root = request_repository_root(&state, query.repo_root.as_deref()).await?;
    let details = workspace::git_commit_details(&root, &query.commit)
        .await
        .map_err(|error| WebError::bad_request(error.to_string()))?;
    Ok(Json(details))
}

/// 读取选中仓库提交 Diff。
///
/// 参数:
/// - `state`: Web 应用状态
/// - `query`: 仓库根目录、提交标识与可选文件路径
///
/// 返回:
/// - 提交 Diff
async fn git_commit_diff(
    State(state): State<WebAppState>,
    Query(query): Query<GitCommitQuery>,
) -> WebResult<Json<workspace::GitDiffResponse>> {
    let root = request_repository_root(&state, query.repo_root.as_deref()).await?;
    let diff = workspace::git_commit_diff(&root, &query.commit, query.path.as_deref())
        .await
        .map_err(|error| WebError::bad_request(error.to_string()))?;
    Ok(Json(diff))
}

/// 读取选中仓库工作树或分支 Diff。
///
/// 参数:
/// - `state`: Web 应用状态
/// - `query`: 仓库根目录、Diff 模式与可选文件路径
///
/// 返回:
/// - 工作树或分支 Diff
async fn git_review_diff(
    State(state): State<WebAppState>,
    Query(query): Query<GitDiffQuery>,
) -> WebResult<Json<workspace::GitDiffResponse>> {
    let root = request_repository_root(&state, query.repo_root.as_deref()).await?;
    let mode = query.mode.as_deref().unwrap_or("working_tree");
    let diff = workspace::git_diff(&root, mode, query.path.as_deref())
        .await
        .map_err(|error| WebError::bad_request(error.to_string()))?;
    Ok(Json(diff))
}

/// 执行选中仓库增强版 Git 操作。
///
/// 参数:
/// - `state`: Web 应用状态
/// - `request`: 仓库根目录、操作名称与参数
///
/// 返回:
/// - Git 输出与刷新后的仓库状态
async fn git_op(
    State(state): State<WebAppState>,
    Json(request): Json<GitOpRequest>,
) -> WebResult<Json<workspace::GitOperationResponse>> {
    let active = state.workspaces.active().map_err(WebError::from)?;
    let root = request_repository_root(&state, request.repo_root.as_deref()).await?;
    let result = workspace::git_op(
        &root,
        workspace::GitOperationRequest {
            action: &request.action,
            path: request.path.as_deref(),
            old_path: request.old_path.as_deref(),
            message: request.message.as_deref(),
            remote_url: request.remote_url.as_deref(),
            branch: request.branch.as_deref(),
            branch_kind: request.branch_kind.as_deref(),
            new_branch: request.new_branch.as_deref(),
            start_point: request.start_point.as_deref(),
            post_action: request.post_action.as_deref(),
            patch: request.patch.as_deref(),
            commit: request.commit.as_deref(),
            reset_mode: request.reset_mode.as_deref(),
            stash_ref: request.stash_ref.as_deref(),
            tag: request.tag.as_deref(),
            remote_name: request.remote_name.as_deref(),
            worktree_path: request.worktree_path.as_deref(),
            workspace_root: Some(&active.path),
            include_untracked: request.include_untracked,
            resolution: request.resolution.as_deref(),
            content: request.content.as_deref(),
            all: request.all,
            amend: request.amend,
            signoff: request.signoff,
            force: request.force,
        },
    )
    .await
    .map_err(|error| WebError::bad_request(error.to_string()))?;
    Ok(Json(result))
}

/// 解析并校验请求使用的仓库工作目录。
///
/// 参数:
/// - `state`: Web 应用状态
/// - `requested`: 可选仓库根目录
///
/// 返回:
/// - 未指定时返回活动工作区，指定时返回校验后的仓库根目录
async fn request_repository_root(
    state: &WebAppState,
    requested: Option<&str>,
) -> WebResult<PathBuf> {
    // 1. 未指定仓库时保持兼容，继续以活动工作区作为 Git 工作目录
    let active = state.workspaces.active().map_err(WebError::from)?;
    let Some(requested) = requested.map(str::trim).filter(|value| !value.is_empty()) else {
        return Ok(PathBuf::from(active.path));
    };
    // 2. 指定仓库时只接受工作区发现范围内的仓库或关联 worktree
    workspace::validate_git_repository_root(Path::new(&active.path), requested)
        .await
        .map_err(|error| WebError::bad_request(error.to_string()))
}
