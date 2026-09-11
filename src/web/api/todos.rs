use super::super::app_state::WebAppState;
use super::super::error::{WebError, WebResult};
use crate::config::AppConfig;
use crate::plugins::todo_view::{TodoSnapshot, TodoView};
use crate::state::{active_session_id_for_workspace, state_dir_for_workspace_session};
use axum::extract::State;
use axum::routing::get;
use axum::{Json, Router};

/// 【待办界面】【路由注册】返回当前会话的待办快照查询入口
/// @returns 不包含写入动作的路由
pub(super) fn routes() -> Router<WebAppState> {
    Router::new().route("/api/todos", get(list))
}

/// 【待办界面】【活动会话】按当前配置执行真实 Lua 快照，查询错误交给统一响应
/// @param state Web 服务及工作区状态
/// @returns 当前清单和历史归档
async fn list(State(state): State<WebAppState>) -> WebResult<Json<TodoSnapshot>> {
    let workspace = state.workspaces.active().map_err(WebError::from)?;
    let workdir = std::path::Path::new(&workspace.path);
    let session = active_session_id_for_workspace(&state.paths, workdir).map_err(WebError::from)?;
    let (_, directory) =
        state_dir_for_workspace_session(&state.paths, workdir, &session).map_err(WebError::from)?;
    let config = AppConfig::load_or_default(&state.paths).map_err(WebError::from)?;
    let view = TodoView::load(&config, &state.paths)
        .await
        .map_err(WebError::from)?;
    Ok(Json(
        view.snapshot(&session, &directory, workdir)
            .await
            .map_err(WebError::from)?,
    ))
}
