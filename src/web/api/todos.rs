use super::super::app_state::WebAppState;
use super::super::error::{WebError, WebResult};
use crate::state::{active_session_id_for_workspace, state_dir_for_workspace_session};
use crate::tools::todo::{TodoHistoryBatch, TodoItem, TodoStore};
use axum::extract::State;
use axum::routing::get;
use axum::{Json, Router};
use serde::Serialize;

/// 当前会话的活动清单和已归档计划。
#[derive(Serialize)]
struct TodoSnapshot {
    items: Vec<TodoItem>,
    history: Vec<TodoHistoryBatch>,
}

/// 返回当前会话的待办快照查询入口。
///
/// 返回:
/// - 不包含写入动作的路由
pub(super) fn routes() -> Router<WebAppState> {
    Router::new().route("/api/todos", get(list))
}

/// 读取当前活动会话的原生待办文件。
///
/// 参数:
/// - `state`: Web 服务及工作区状态
///
/// 返回:
/// - 当前清单和历史归档
async fn list(State(state): State<WebAppState>) -> WebResult<Json<TodoSnapshot>> {
    let workspace = state.workspaces.active().map_err(WebError::from)?;
    let workdir = std::path::Path::new(&workspace.path);
    let session = active_session_id_for_workspace(&state.paths, workdir).map_err(WebError::from)?;
    let (_, directory) =
        state_dir_for_workspace_session(&state.paths, workdir, &session).map_err(WebError::from)?;
    let store = TodoStore::new(directory.join("todos.json"));
    Ok(Json(TodoSnapshot {
        items: store.list().map_err(WebError::from)?,
        history: store.list_history().map_err(WebError::from)?,
    }))
}
