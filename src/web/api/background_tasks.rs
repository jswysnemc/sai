use super::super::app_state::WebAppState;
use super::super::error::{WebError, WebResult};
use crate::config::AppConfig;
use crate::tools::command::{
    cleanup_managed_background_tasks, list_managed_background_tasks,
    read_managed_background_output, stop_background_task_for_user,
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

#[derive(Debug, Default, Deserialize)]
struct StopQuery {
    #[serde(default)]
    force: bool,
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
    let output = list_managed_background_tasks(&state.paths, &config)
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
    let output =
        read_managed_background_output(&state.paths, &config, &id, query.tail_lines.clamp(1, 2000))
            .await
            .map_err(WebError::from)?;
    Ok(Json(parse_tool_output(output)?))
}

/// 【后台任务】【停止接口】停止指定任务，force 为真时跳过宽限期。
/// 参数: state 为应用状态，id 为任务标识，query 为停止模式；返回: 真实任务状态。
async fn stop(
    State(state): State<WebAppState>,
    Path(id): Path<String>,
    Query(query): Query<StopQuery>,
) -> WebResult<Json<Value>> {
    let config = load_config(&state)?;
    let output = stop_background_task_for_user(&state.paths, &config, &id, query.force)
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
    let output = cleanup_managed_background_tasks(&state.paths, &config, query.remove_logs)
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

    /// 【后台任务测试】【停止模式】旧请求默认温和停止，显式参数才强制终止。
    /// 参数: 无；返回: 无。
    #[test]
    fn stop_defaults_to_graceful() {
        assert!(!serde_json::from_str::<StopQuery>("{}").unwrap().force);
        assert!(
            serde_json::from_str::<StopQuery>(r#"{"force":true}"#)
                .unwrap()
                .force
        );
    }

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
