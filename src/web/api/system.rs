use super::super::app_state::WebAppState;
use super::super::error::{WebError, WebResult};
use crate::config::AppConfig;
use crate::state::StateStore;
use anyhow::{bail, Result};
use axum::extract::{Query, State};
use axum::routing::get;
use axum::{Json, Router};
use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Deserialize)]
struct SystemUsageQuery {
    provider_id: Option<String>,
    model: Option<String>,
}

#[derive(Serialize)]
struct SystemUsageResponse {
    session: SessionUsageResponse,
    process: ProcessUsageResponse,
    runtime: RuntimeUsageResponse,
}

#[derive(Serialize)]
struct SessionUsageResponse {
    id: String,
    requests: u64,
    prompt_tokens: u64,
    completion_tokens: u64,
    total_tokens: u64,
    turn_count: usize,
    context_prompt_tokens: usize,
    context_window_tokens: usize,
    context_token_ratio: f32,
    tool_calls: usize,
    checkpoint_count: usize,
    compacted_turns: usize,
    latest_checkpoint_at: Option<String>,
    latest_checkpoint_reason: Option<String>,
    compaction_warning: Option<String>,
    context_breakdown: ContextUsageBreakdownResponse,
}

#[derive(Serialize, Default)]
struct ContextUsageBreakdownResponse {
    system_prompt_tokens: usize,
    tools_and_agents_tokens: usize,
    conversation_tokens: usize,
    connectors_and_mcp_tokens: usize,
    skills_tokens: usize,
}

#[derive(Serialize)]
struct ProcessUsageResponse {
    pid: u32,
    uptime_seconds: u64,
    rss_bytes: Option<u64>,
    cpu_percent: f64,
}

#[derive(Serialize)]
struct RuntimeUsageResponse {
    active_run: bool,
    terminal_count: usize,
}

/// 返回系统用量路由。
///
/// 返回:
/// - 系统用量 API 路由
pub(super) fn routes() -> Router<WebAppState> {
    Router::new().route("/api/system/usage", get(usage))
}

/// 聚合当前会话、进程和 Web 运行时用量。
///
/// 参数:
/// - `state`: Web 应用状态
/// - `query`: 主界面当前选择的供应商和模型
///
/// 返回:
/// - 系统用量快照
async fn usage(
    State(state): State<WebAppState>,
    Query(query): Query<SystemUsageQuery>,
) -> WebResult<Json<SystemUsageResponse>> {
    let config = AppConfig::load_or_default(&state.paths).map_err(WebError::from)?;
    let context_window_tokens = usage_context_window(&config, &query).map_err(WebError::from)?;
    let store = StateStore::new(&state.paths).map_err(WebError::from)?;
    // 用量顶栏不应因瞬时 DB 忙碌打挂；快照失败时降级为零值并带警告
    let snapshot = match store.session_snapshot(context_window_tokens) {
        Ok(snapshot) => snapshot,
        Err(error) => crate::state::SessionSnapshot {
            session_id: store.session_id().to_string(),
            turn_count: 0,
            checkpoint_count: 0,
            checkpoint_covered_turns: 0,
            tail_turns: 0,
            latest_checkpoint_at: None,
            latest_checkpoint_reason: None,
            context_chars: 0,
            context_limit_chars: context_window_tokens,
            context_ratio: 0.0,
            context_prompt_tokens: 0,
            context_window_tokens,
            context_token_ratio: 0.0,
            usage: crate::state::UsageSnapshot::default(),
            compaction: None,
            recovery: crate::state::RecoverySnapshot::default(),
            context_epoch: None,
            session_memory: None,
            tool_history: crate::state::ToolHistorySummary::default(),
            runtime_recovery: crate::runtime_recovery::RuntimeRecoverySummary::default(),
            dynamic_sources: Vec::new(),
            projection_warnings: vec![format!("usage snapshot unavailable: {error}")],
            active_run: None,
            last_turn_duration_ms: 0,
        },
    };
    let process = state.system_monitor.snapshot();
    let terminal_count = state.terminals.list().map_err(WebError::from)?.len();
    let workspace = state.workspaces.active().map_err(WebError::from)?;
    let active_run = state
        .runs
        .is_session_active(&workspace.id, &snapshot.session_id)
        .await;
    // 【Web主界面】【上下文分项】估算系统提示、工具、对话、MCP、技能占用
    let breakdown = match super::super::services::context_breakdown::estimate_context_breakdown(
        &config,
        &state.paths,
        &store,
    ) {
        Ok(value) => value,
        Err(_error) => super::super::services::context_breakdown::ContextUsageBreakdown::default(),
    };
    Ok(Json(SystemUsageResponse {
        session: SessionUsageResponse {
            id: snapshot.session_id,
            requests: snapshot.usage.requests,
            prompt_tokens: snapshot.usage.prompt_tokens,
            completion_tokens: snapshot.usage.completion_tokens,
            total_tokens: snapshot.usage.total_tokens,
            turn_count: snapshot.turn_count,
            context_prompt_tokens: snapshot.context_prompt_tokens,
            context_window_tokens: snapshot.context_window_tokens,
            context_token_ratio: snapshot.context_token_ratio,
            tool_calls: snapshot.tool_history.call_count,
            checkpoint_count: snapshot.checkpoint_count,
            compacted_turns: snapshot.checkpoint_covered_turns,
            latest_checkpoint_at: snapshot.latest_checkpoint_at,
            latest_checkpoint_reason: snapshot.latest_checkpoint_reason,
            compaction_warning: (snapshot.checkpoint_count >= 2).then(|| {
                    "conversation has been compacted multiple times; start a focused session if details become distorted"
                        .to_string()
                }),
            context_breakdown: ContextUsageBreakdownResponse {
                system_prompt_tokens: breakdown.system_prompt_tokens,
                tools_and_agents_tokens: breakdown.tools_and_agents_tokens,
                conversation_tokens: breakdown.conversation_tokens,
                connectors_and_mcp_tokens: breakdown.connectors_and_mcp_tokens,
                skills_tokens: breakdown.skills_tokens,
            },
        },
        process: ProcessUsageResponse {
            pid: process.pid,
            uptime_seconds: process.uptime_seconds,
            rss_bytes: process.rss_bytes,
            cpu_percent: process.cpu_percent,
        },
        runtime: RuntimeUsageResponse {
            active_run,
            terminal_count,
        },
    }))
}

/// 解析系统用量对应的模型上下文容量。
///
/// 参数:
/// - `config`: 应用配置
/// - `query`: 主界面当前模型查询参数
///
/// 返回:
/// - 当前模型上下文 token 数
fn usage_context_window(config: &AppConfig, query: &SystemUsageQuery) -> Result<usize> {
    match (&query.provider_id, &query.model) {
        (None, None) => config.active_context_window_tokens(),
        (Some(provider_id), Some(model)) => {
            // 【Web主界面】【同步模型上下文】1. 校验供应商和模型必须同时为非空值
            if provider_id.trim().is_empty() || model.trim().is_empty() {
                bail!("provider_id and model cannot be empty");
            }
            // 【Web主界面】【同步模型上下文】2. 在临时配置中应用选择，复用统一的上下文容量解析规则
            let mut selected_config = config.clone();
            selected_config.set_active_provider_model(provider_id, model)?;
            selected_config.active_context_window_tokens()
        }
        _ => bail!("provider_id and model must be provided together"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selected_model_controls_system_usage_context_window() {
        let mut config = AppConfig::default();
        let provider_id = config.active_provider.clone();
        let provider = config.provider(Some(&provider_id)).unwrap();
        let default_model = provider.default_model.clone();
        config
            .providers
            .iter_mut()
            .find(|provider| provider.id == provider_id)
            .unwrap()
            .set_model_context_chars_for(&default_model, Some(64_000));
        config
            .providers
            .iter_mut()
            .find(|provider| provider.id == provider_id)
            .unwrap()
            .set_model_context_chars_for("large-model", Some(256_000));
        let query = SystemUsageQuery {
            provider_id: Some(provider_id),
            model: Some("large-model".to_string()),
        };

        assert_eq!(usage_context_window(&config, &query).unwrap(), 256_000);
    }

    #[test]
    fn system_usage_rejects_partial_model_selection() {
        let query = SystemUsageQuery {
            provider_id: Some("provider-a".to_string()),
            model: None,
        };

        let error = usage_context_window(&AppConfig::default(), &query).unwrap_err();

        assert!(error
            .to_string()
            .contains("provider_id and model must be provided together"));
    }
}
