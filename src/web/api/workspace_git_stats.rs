use super::{
    request_repository_root, workspace, GitRepositoryQuery, WebAppState, WebError, WebResult,
};
use axum::extract::{Query, State};
use axum::routing::get;
use axum::{Json, Router};

/// 【工作概览】【统计接口】注册只返回数值的 Git 改动接口；无参数，返回受保护路由。
pub(super) fn routes() -> Router<WebAppState> {
    Router::new().route("/api/workspace/git/diff-stats", get(git_diff_stats))
}

/// 【工作概览】【统计查询】在已校验的仓库内统计工作树改动。
/// 参数：`state` 为应用状态，`query` 为可选仓库位置；返回增删行数的 JSON 响应。
async fn git_diff_stats(
    State(state): State<WebAppState>,
    Query(query): Query<GitRepositoryQuery>,
) -> WebResult<Json<workspace::GitDiffStats>> {
    let root = request_repository_root(&state, query.repo_root.as_deref()).await?;
    let stats = workspace::git_diff_stats(&root)
        .await
        .map_err(|error| WebError::bad_request(error.to_string()))?;
    Ok(Json(stats))
}
