use super::super::app_state::WebAppState;
use super::super::error::{WebError, WebResult};
use crate::goal::GoalStatus;
use crate::state::StateStore;
use axum::extract::{Path, State};
use axum::routing::get;
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::{json, Value};

#[derive(Deserialize)]
struct SetGoalRequest {
    objective: String,
    token_budget: Option<u64>,
}

#[derive(Deserialize)]
struct UpdateGoalRequest {
    status: Option<String>,
    objective: Option<String>,
    #[serde(default, deserialize_with = "deserialize_double_option")]
    token_budget: Option<Option<u64>>,
}

/// 返回会话 Goal 管理路由。
///
/// 返回:
/// - Goal API 路由
pub(super) fn routes() -> Router<WebAppState> {
    Router::new().route(
        "/api/sessions/:id/goal",
        get(read).put(set).patch(update).delete(clear),
    )
}

/// 读取会话当前目标。
///
/// 参数:
/// - `state`: Web 应用状态
/// - `id`: 会话 ID
///
/// 返回:
/// - 当前目标
async fn read(State(state): State<WebAppState>, Path(id): Path<String>) -> WebResult<Json<Value>> {
    let store = session_state(&state, &id)?;
    Ok(Json(
        json!({ "goal": store.goal().map_err(WebError::from)? }),
    ))
}

/// 创建会话目标。
///
/// 参数:
/// - `state`: Web 应用状态
/// - `id`: 会话 ID
/// - `request`: 目标文本和可选预算
///
/// 返回:
/// - 新目标
async fn set(
    State(state): State<WebAppState>,
    Path(id): Path<String>,
    Json(request): Json<SetGoalRequest>,
) -> WebResult<Json<Value>> {
    let store = session_state(&state, &id)?;
    let goal = store
        .replace_goal(&request.objective, request.token_budget, false)
        .map_err(|error| WebError::bad_request(error.to_string()))?;
    Ok(Json(json!({ "goal": goal })))
}

/// 更新会话目标状态。
///
/// 参数:
/// - `state`: Web 应用状态
/// - `id`: 会话 ID
/// - `request`: 新状态
///
/// 返回:
/// - 更新后的目标
async fn update(
    State(state): State<WebAppState>,
    Path(id): Path<String>,
    Json(request): Json<UpdateGoalRequest>,
) -> WebResult<Json<Value>> {
    if request.status.is_none() && request.objective.is_none() && request.token_budget.is_none() {
        return Err(WebError::bad_request("goal update cannot be empty"));
    }
    let status = request
        .status
        .as_deref()
        .map(GoalStatus::parse)
        .transpose()
        .map_err(|error| WebError::bad_request(error.to_string()))?;
    if matches!(
        status,
        Some(GoalStatus::BudgetLimited | GoalStatus::UsageLimited)
    ) {
        return Err(WebError::bad_request(
            "usage_limited and budget_limited are managed by goal accounting".to_string(),
        ));
    }
    let store = session_state(&state, &id)?;
    let goal = store
        .update_goal(request.objective.as_deref(), request.token_budget, status)
        .map_err(|error| WebError::bad_request(error.to_string()))?;
    Ok(Json(json!({ "goal": goal })))
}

/// 区分缺失、null 和正整数预算字段。
///
/// 参数:
/// - `deserializer`: Serde 字段反序列化器
///
/// 返回:
/// - 外层表示字段是否存在，内层表示是否设置预算
fn deserialize_double_option<'de, D>(deserializer: D) -> Result<Option<Option<u64>>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    Option::<u64>::deserialize(deserializer).map(Some)
}

/// 清除会话目标。
///
/// 参数:
/// - `state`: Web 应用状态
/// - `id`: 会话 ID
///
/// 返回:
/// - 是否清除了目标
async fn clear(State(state): State<WebAppState>, Path(id): Path<String>) -> WebResult<Json<Value>> {
    let store = session_state(&state, &id)?;
    let cleared = store.clear_goal().map_err(WebError::from)?;
    Ok(Json(json!({ "cleared": cleared })))
}

/// 打开当前活动工作区中的指定会话状态。
///
/// 参数:
/// - `state`: Web 应用状态
/// - `session_id`: 会话 ID
///
/// 返回:
/// - 会话状态存储
fn session_state(state: &WebAppState, session_id: &str) -> WebResult<StateStore> {
    let workspace = state.workspaces.active().map_err(WebError::from)?;
    StateStore::for_workspace_session(
        &state.paths,
        std::path::Path::new(&workspace.path),
        session_id,
    )
    .map_err(|error| WebError::not_found(error.to_string()))
}

#[cfg(test)]
mod tests {
    use super::UpdateGoalRequest;

    #[test]
    fn distinguishes_missing_and_null_token_budget() {
        let missing: UpdateGoalRequest = serde_json::from_str(r#"{"status":"active"}"#).unwrap();
        let cleared: UpdateGoalRequest = serde_json::from_str(r#"{"token_budget":null}"#).unwrap();
        let set: UpdateGoalRequest = serde_json::from_str(r#"{"token_budget":1200}"#).unwrap();

        assert_eq!(missing.token_budget, None);
        assert_eq!(cleared.token_budget, Some(None));
        assert_eq!(set.token_budget, Some(Some(1200)));
    }
}
