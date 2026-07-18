use super::super::app_state::WebAppState;
use super::super::error::{WebError, WebResult};
use super::super::workspace;
use axum::extract::{Query, State};
use axum::http::header::CONTENT_TYPE;
use axum::http::HeaderValue;
use axum::response::Response;
use axum::routing::get;
use axum::{Json, Router};
use serde::Deserialize;

#[derive(Deserialize)]
struct TreeQuery {
    path: Option<String>,
    depth: Option<usize>,
}

#[derive(Deserialize)]
struct FileQuery {
    path: String,
}

#[derive(Deserialize)]
struct SaveFileRequest {
    path: String,
    content: String,
    expected_modified_at: Option<u64>,
}

#[derive(Deserialize)]
struct CreateEntryRequest {
    path: String,
    kind: String,
}

#[derive(Deserialize)]
struct RenameEntryRequest {
    from: String,
    to: String,
}

#[derive(Deserialize)]
struct DeleteEntryRequest {
    path: String,
}

#[derive(Deserialize)]
struct GitActionRequest {
    action: String,
    #[serde(default)]
    paths: Vec<String>,
    message: Option<String>,
}

#[derive(Deserialize)]
struct GitOpRequest {
    action: String,
    path: Option<String>,
    old_path: Option<String>,
    message: Option<String>,
    remote_url: Option<String>,
    branch: Option<String>,
    branch_kind: Option<String>,
    new_branch: Option<String>,
    start_point: Option<String>,
    #[serde(default)]
    force: bool,
}

#[derive(Deserialize)]
struct GitDiffQuery {
    mode: Option<String>,
    path: Option<String>,
}

#[derive(Deserialize)]
struct GitLogQuery {
    limit: Option<usize>,
    skip: Option<usize>,
}

#[derive(Deserialize)]
struct GitCommitQuery {
    commit: String,
    path: Option<String>,
}

/// 返回工作区文件与 Diff 路由。
pub(super) fn routes() -> Router<WebAppState> {
    Router::new()
        .route("/api/workspace/tree", get(tree))
        .route("/api/workspace/file", get(file).put(save_file))
        .route("/api/workspace/image", get(image))
        .route(
            "/api/workspace/entry",
            axum::routing::post(create_entry)
                .patch(rename_entry)
                .delete(delete_entry),
        )
        .route("/api/workspace/diff", get(diff))
        .route("/api/workspace/git", axum::routing::post(git_action))
        .route("/api/workspace/git/status", get(git_status))
        .route("/api/workspace/git/branches", get(git_branches))
        .route("/api/workspace/git/log", get(git_log))
        .route("/api/workspace/git/commit", get(git_commit_details))
        .route("/api/workspace/git/commit-diff", get(git_commit_diff))
        .route("/api/workspace/git/diff", get(git_review_diff))
        .route("/api/workspace/git/op", axum::routing::post(git_op))
}

/// 返回用于编辑器预览的图像文件。
async fn image(
    State(state): State<WebAppState>,
    Query(query): Query<FileQuery>,
) -> WebResult<Response> {
    let active = state.workspaces.active().map_err(WebError::from)?;
    let image = workspace::read_image(std::path::Path::new(&active.path), &query.path)
        .map_err(|error| WebError::bad_request(error.to_string()))?;
    let content_type = HeaderValue::from_str(&image.mime)
        .map_err(|error| WebError::bad_request(error.to_string()))?;
    Response::builder()
        .header(CONTENT_TYPE, content_type)
        .body(axum::body::Body::from(image.bytes))
        .map_err(anyhow::Error::from)
        .map_err(WebError::from)
}

/// 读取文件树。
async fn tree(
    State(state): State<WebAppState>,
    Query(query): Query<TreeQuery>,
) -> WebResult<Json<Vec<workspace::FileNode>>> {
    let active = state.workspaces.active().map_err(WebError::from)?;
    let nodes = workspace::read_tree(
        std::path::Path::new(&active.path),
        query.path.as_deref().unwrap_or(""),
        query.depth.unwrap_or(4),
    )
    .map_err(|error| WebError::bad_request(error.to_string()))?;
    Ok(Json(nodes))
}

/// 读取文本文件。
async fn file(
    State(state): State<WebAppState>,
    Query(query): Query<FileQuery>,
) -> WebResult<Json<workspace::FileContent>> {
    let active = state.workspaces.active().map_err(WebError::from)?;
    let file = workspace::read_file(std::path::Path::new(&active.path), &query.path)
        .map_err(|error| WebError::bad_request(error.to_string()))?;
    Ok(Json(file))
}

/// 保存文本文件。
async fn save_file(
    State(state): State<WebAppState>,
    Json(request): Json<SaveFileRequest>,
) -> WebResult<Json<workspace::FileContent>> {
    let active = state.workspaces.active().map_err(WebError::from)?;
    if let Some(expected) = request.expected_modified_at {
        let current = workspace::read_file(std::path::Path::new(&active.path), &request.path)
            .map_err(|error| WebError::bad_request(error.to_string()))?;
        if current.modified_at != Some(expected) {
            return Err(WebError::conflict(
                "file changed outside the editor; review the latest content before saving",
            ));
        }
    }
    let file = workspace::write_file(
        std::path::Path::new(&active.path),
        &request.path,
        &request.content,
    )
    .map_err(|error| WebError::bad_request(error.to_string()))?;
    Ok(Json(file))
}

/// 创建工作区文件或目录。
async fn create_entry(
    State(state): State<WebAppState>,
    Json(request): Json<CreateEntryRequest>,
) -> WebResult<Json<workspace::FileMutation>> {
    let active = state.workspaces.active().map_err(WebError::from)?;
    let entry = workspace::create_entry(
        std::path::Path::new(&active.path),
        &request.path,
        request.kind == "directory",
    )
    .map_err(|error| WebError::bad_request(error.to_string()))?;
    Ok(Json(entry))
}

/// 重命名工作区文件或目录。
async fn rename_entry(
    State(state): State<WebAppState>,
    Json(request): Json<RenameEntryRequest>,
) -> WebResult<Json<workspace::FileMutation>> {
    let active = state.workspaces.active().map_err(WebError::from)?;
    let entry = workspace::rename_entry(
        std::path::Path::new(&active.path),
        &request.from,
        &request.to,
    )
    .map_err(|error| WebError::bad_request(error.to_string()))?;
    Ok(Json(entry))
}

/// 删除工作区文件或目录。
async fn delete_entry(
    State(state): State<WebAppState>,
    Json(request): Json<DeleteEntryRequest>,
) -> WebResult<Json<workspace::FileMutation>> {
    let active = state.workspaces.active().map_err(WebError::from)?;
    let entry = workspace::delete_entry(std::path::Path::new(&active.path), &request.path)
        .map_err(|error| WebError::bad_request(error.to_string()))?;
    Ok(Json(entry))
}

/// 读取当前工作区 Git Diff。
async fn diff(State(state): State<WebAppState>) -> WebResult<Json<workspace::GitDiff>> {
    let active = state.workspaces.active().map_err(WebError::from)?;
    let diff = workspace::read_git_diff(std::path::Path::new(&active.path))
        .await
        .map_err(WebError::from)?;
    Ok(Json(diff))
}

/// 执行兼容版 Git 暂存、取消暂存、撤销或提交操作。
async fn git_action(
    State(state): State<WebAppState>,
    Json(request): Json<GitActionRequest>,
) -> WebResult<Json<workspace::GitDiff>> {
    let active = state.workspaces.active().map_err(WebError::from)?;
    let git = workspace::apply_git_action(
        std::path::Path::new(&active.path),
        &request.action,
        &request.paths,
        request.message.as_deref(),
    )
    .await
    .map_err(|error| WebError::bad_request(error.to_string()))?;
    Ok(Json(git))
}

/// 读取增强版 Git 状态。
async fn git_status(
    State(state): State<WebAppState>,
) -> WebResult<Json<workspace::GitRepositoryState>> {
    let active = state.workspaces.active().map_err(WebError::from)?;
    let status = workspace::git_status(std::path::Path::new(&active.path))
        .await
        .map_err(WebError::from)?;
    Ok(Json(status))
}

/// 读取分支列表。
async fn git_branches(
    State(state): State<WebAppState>,
) -> WebResult<Json<workspace::GitBranchesResponse>> {
    let active = state.workspaces.active().map_err(WebError::from)?;
    let branches = workspace::git_branches(std::path::Path::new(&active.path))
        .await
        .map_err(WebError::from)?;
    Ok(Json(branches))
}

/// 读取提交历史。
async fn git_log(
    State(state): State<WebAppState>,
    Query(query): Query<GitLogQuery>,
) -> WebResult<Json<workspace::GitLogResponse>> {
    let active = state.workspaces.active().map_err(WebError::from)?;
    let log = workspace::git_log(std::path::Path::new(&active.path), query.limit, query.skip)
        .await
        .map_err(WebError::from)?;
    Ok(Json(log))
}

/// 读取提交详情。
async fn git_commit_details(
    State(state): State<WebAppState>,
    Query(query): Query<GitCommitQuery>,
) -> WebResult<Json<workspace::GitCommitDetailsResponse>> {
    let active = state.workspaces.active().map_err(WebError::from)?;
    let details = workspace::git_commit_details(std::path::Path::new(&active.path), &query.commit)
        .await
        .map_err(|error| WebError::bad_request(error.to_string()))?;
    Ok(Json(details))
}

/// 读取提交 Diff。
async fn git_commit_diff(
    State(state): State<WebAppState>,
    Query(query): Query<GitCommitQuery>,
) -> WebResult<Json<workspace::GitDiffResponse>> {
    let active = state.workspaces.active().map_err(WebError::from)?;
    let diff = workspace::git_commit_diff(
        std::path::Path::new(&active.path),
        &query.commit,
        query.path.as_deref(),
    )
    .await
    .map_err(|error| WebError::bad_request(error.to_string()))?;
    Ok(Json(diff))
}

/// 读取工作树或分支 Diff。
async fn git_review_diff(
    State(state): State<WebAppState>,
    Query(query): Query<GitDiffQuery>,
) -> WebResult<Json<workspace::GitDiffResponse>> {
    let active = state.workspaces.active().map_err(WebError::from)?;
    let mode = query.mode.as_deref().unwrap_or("working_tree");
    let diff = workspace::git_diff(
        std::path::Path::new(&active.path),
        mode,
        query.path.as_deref(),
    )
    .await
    .map_err(|error| WebError::bad_request(error.to_string()))?;
    Ok(Json(diff))
}

/// 执行增强版 Git 操作。
async fn git_op(
    State(state): State<WebAppState>,
    Json(request): Json<GitOpRequest>,
) -> WebResult<Json<workspace::GitOperationResponse>> {
    let active = state.workspaces.active().map_err(WebError::from)?;
    let result = workspace::git_op(
        std::path::Path::new(&active.path),
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
            force: request.force,
        },
    )
    .await
    .map_err(|error| WebError::bad_request(error.to_string()))?;
    Ok(Json(result))
}
