use super::super::app_state::WebAppState;
use super::super::error::{WebError, WebResult};
use crate::tools::command::{BackgroundCommandStore, BackgroundCommandTask};
use crate::tools::subagent_state::list_subagents_for_owner;
use axum::extract::{Path, State};
use axum::routing::get;
use axum::{Json, Router};
use serde::Serialize;
use serde_json::{json, Value};

/// 关闭前提示用的一项后台工作。
#[derive(Debug, Serialize)]
struct BackgroundWorkItem {
    kind: &'static str,
    id: String,
    label: String,
}

/// 返回后台工作查询路由。
///
/// 只按会话查询。终端会话和网页会话并行存在，关闭其中一个不能看到另一个的后台工作。
pub(super) fn routes() -> Router<WebAppState> {
    Router::new().route("/api/sessions/:id/background-work", get(session_work))
}

/// 列出指定会话仍在运行的子智能体和后台命令。
///
/// 参数:
/// - `state`: Web 应用状态
/// - `id`: 会话标识
///
/// 返回:
/// - 运行中的工作列表
async fn session_work(State(state): State<WebAppState>, Path(id): Path<String>) -> WebResult<Json<Value>> {
    let (_, state_dir) = crate::state::locate_session_dirs(&state.paths, &id)
        .map_err(|error| WebError::not_found(error.to_string()))?;
    let items = running_work(&state, &id, &state_dir.display().to_string());
    Ok(Json(json!({ "items": items })))
}

/// 收集一个会话的运行中工作。
///
/// 参数:
/// - `state`: Web 应用状态
/// - `session_id`: 会话标识
/// - `owner_key`: 子智能体归属键
///
/// 返回:
/// - 工作列表
fn running_work(state: &WebAppState, session_id: &str, owner_key: &str) -> Vec<BackgroundWorkItem> {
    let mut items = list_subagents_for_owner(owner_key)
        .into_iter()
        .filter(|snapshot| snapshot.status == "running")
        .map(|snapshot| BackgroundWorkItem {
            kind: "subagent",
            id: snapshot.id,
            label: work_label(&snapshot.description, &snapshot.subagent_type),
        })
        .collect::<Vec<_>>();
    items.extend(running_commands(state, session_id).into_iter().map(|task| {
        let id = task.id.clone();
        BackgroundWorkItem {
            kind: "command",
            id,
            label: command_label(task),
        }
    }));
    items
}

/// 读取指定会话仍在运行的后台命令。
///
/// 参数:
/// - `state`: Web 应用状态
/// - `session_id`: 会话标识
///
/// 返回:
/// - 归属该会话的运行中命令
fn running_commands(state: &WebAppState, session_id: &str) -> Vec<BackgroundCommandTask> {
    BackgroundCommandStore::new(state.paths.state_dir.clone())
        .load()
        .unwrap_or_default()
        .into_iter()
        .filter(|task| task.status == "running")
        .filter(|task| task.owned_by_session(session_id))
        .filter(|task| task.runtime_owner_kind.as_deref() != Some("gateway"))
        .collect()
}

/// 取非空展示名。
///
/// 参数:
/// - `primary`: 优先文案
/// - `fallback`: 空文案时的替代
///
/// 返回:
/// - 展示名
fn work_label(primary: &str, fallback: &str) -> String {
    let primary = primary.trim();
    if primary.is_empty() { fallback.to_string() } else { primary.to_string() }
}

/// 取后台命令短标签。
///
/// 参数:
/// - `task`: 后台命令
///
/// 返回:
/// - 标签或命令前 48 个字符
fn command_label(task: BackgroundCommandTask) -> String {
    work_label(&task.label, &task.command.chars().take(48).collect::<String>())
}
