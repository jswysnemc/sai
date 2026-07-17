use super::super::app_state::WebAppState;
use super::super::error::{WebError, WebResult};
use crate::config::AppConfig;
use crate::tools::command::{
    cleanup_background_tasks_for_user, list_background_tasks_for_user,
    read_background_task_output_for_user, stop_background_task_for_user,
};
use axum::extract::{Path, Query, State};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::Value;

#[derive(Debug, Deserialize)]
struct OutputQuery {
    #[serde(default = "default_tail_lines")]
    tail_lines: usize,
}

#[derive(Debug, Deserialize)]
struct CleanupQuery {
    #[serde(default)]
    remove_logs: bool,
}

/// 返回后台任务管理路由。
pub(super) fn routes() -> Router<WebAppState> {
    Router::new()
        .route("/api/background-tasks", get(list).delete(cleanup))
        .route("/api/background-tasks/:id/output", get(output))
        .route("/api/background-tasks/:id/stop", post(stop))
}

/// 列出全部后台任务并刷新运行状态。
async fn list(State(state): State<WebAppState>) -> WebResult<Json<Value>> {
    let config = load_config(&state)?;
    // 1. 先迁移历史遗留的网关记录，网关进程只在网关管理页展示
    crate::gateways::process_control::migrate_legacy_gateway_tasks(&state.paths)
        .map_err(WebError::from)?;
    let output = list_background_tasks_for_user(&state.paths, &config)
        .await
        .map_err(WebError::from)?;
    Ok(Json(parse_tool_output(output)?))
}

/// 读取指定后台任务的标准输出和错误输出尾部。
async fn output(
    State(state): State<WebAppState>,
    Path(id): Path<String>,
    Query(query): Query<OutputQuery>,
) -> WebResult<Json<Value>> {
    let config = load_config(&state)?;
    let output = read_background_task_output_for_user(
        &state.paths,
        &config,
        &id,
        "all",
        query.tail_lines.clamp(1, 2000),
    )
    .await
    .map_err(WebError::from)?;
    Ok(Json(parse_tool_output(output)?))
}

/// 停止指定后台任务，宽限期后仍未退出时由底层实现强制终止。
async fn stop(State(state): State<WebAppState>, Path(id): Path<String>) -> WebResult<Json<Value>> {
    let config = load_config(&state)?;
    let output = stop_background_task_for_user(&state.paths, &config, &id, false)
        .await
        .map_err(WebError::from)?;
    Ok(Json(parse_tool_output(output)?))
}

/// 清理全部已结束任务记录，可选择同时删除日志文件。
async fn cleanup(
    State(state): State<WebAppState>,
    Query(query): Query<CleanupQuery>,
) -> WebResult<Json<Value>> {
    let config = load_config(&state)?;
    let output = cleanup_background_tasks_for_user(&state.paths, &config, query.remove_logs)
        .await
        .map_err(WebError::from)?;
    Ok(Json(parse_tool_output(output)?))
}

/// 加载后台任务操作需要的当前应用配置。
fn load_config(state: &WebAppState) -> WebResult<AppConfig> {
    AppConfig::load_or_default(&state.paths).map_err(WebError::from)
}

/// 将命令模块返回的 JSON 文本转换为 Web API 响应值。
fn parse_tool_output(output: String) -> WebResult<Value> {
    serde_json::from_str(&output)
        .map_err(anyhow::Error::from)
        .map_err(WebError::from)
}

/// 返回默认读取的日志尾部行数。
fn default_tail_lines() -> usize {
    200
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_background_task_tool_output() {
        let value = parse_tool_output(r#"{"ok":true,"tasks":[]}"#.to_string()).unwrap();
        assert_eq!(value["ok"], true);
        assert_eq!(value["tasks"].as_array().map(Vec::len), Some(0));
    }

    #[test]
    fn rejects_invalid_background_task_tool_output() {
        assert!(parse_tool_output("not-json".to_string()).is_err());
    }
}
