use super::super::app_state::WebAppState;
use super::super::error::{WebError, WebResult};
use super::super::workspace::{self, GitWatchEvent, RepositoryWatcher};
use axum::extract::{Query, State};
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::routing::get;
use axum::Router;
use futures_util::stream::{self, Stream};
use serde::Deserialize;
use std::convert::Infallible;
use std::path::{Path, PathBuf};
use std::time::Duration;

#[derive(Deserialize)]
struct GitEventsQuery {
    repo_root: Option<String>,
}

/// 返回 Git 文件变化事件路由。
///
/// 返回:
/// - 仓库变化 SSE 路由
pub(super) fn routes() -> Router<WebAppState> {
    Router::new().route("/api/workspace/git/events", get(events))
}

/// 实时推送工作区和选中仓库变化。
///
/// 参数:
/// - `state`: Web 应用状态
/// - `query`: 可选仓库或 worktree 根目录
///
/// 返回:
/// - 持续输出 Git 变化事件的 SSE 响应
async fn events(
    State(state): State<WebAppState>,
    Query(query): Query<GitEventsQuery>,
) -> WebResult<Sse<impl Stream<Item = Result<Event, Infallible>>>> {
    let active = state.workspaces.active().map_err(WebError::from)?;
    let repository_root =
        requested_repository_root(&active.path, query.repo_root.as_deref()).await?;
    let watcher = RepositoryWatcher::start(Path::new(&active.path), repository_root.as_deref())
        .await
        .map_err(WebError::from)?;
    let event_stream = stream::unfold(watcher, |mut watcher| async move {
        let event = watcher.next_event().await?;
        let item = Ok::<_, Infallible>(git_sse_event(&event));
        Some((item, watcher))
    });
    Ok(Sse::new(event_stream).keep_alive(
        KeepAlive::new()
            .interval(Duration::from_secs(15))
            .text("keep-alive"),
    ))
}

/// 校验事件订阅中的仓库根目录。
///
/// 参数:
/// - `workspace_root`: 活动工作区目录
/// - `requested`: 可选仓库根目录
///
/// 返回:
/// - 未指定时返回 `None`，指定时返回允许访问的仓库路径
async fn requested_repository_root(
    workspace_root: &str,
    requested: Option<&str>,
) -> WebResult<Option<PathBuf>> {
    let Some(requested) = requested.map(str::trim).filter(|value| !value.is_empty()) else {
        return Ok(None);
    };
    workspace::validate_git_repository_root(Path::new(workspace_root), requested)
        .await
        .map(Some)
        .map_err(|error| WebError::bad_request(error.to_string()))
}

/// 将 Git 变化事件编码为 SSE。
///
/// 参数:
/// - `event`: 仓库变化事件
///
/// 返回:
/// - 带序号、类型和 JSON 数据的 SSE 事件
fn git_sse_event(event: &GitWatchEvent) -> Event {
    let kind = if event.error.is_some() {
        "git.error"
    } else {
        "git.changed"
    };
    Event::default()
        .id(event.sequence.to_string())
        .event(kind)
        .data(serde_json::to_string(event).unwrap_or_else(|_| "{}".to_string()))
}
