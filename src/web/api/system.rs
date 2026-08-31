use super::super::app_state::WebAppState;
use super::super::error::{WebError, WebResult};
use crate::agent::AgentMode;
use crate::config::AppConfig;
use crate::llm::Usage;
use crate::state::StateStore;
use anyhow::{bail, Result};
use axum::extract::{Query, State};
use axum::routing::get;
use axum::{Json, Router};
use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Deserialize)]
struct SystemUsageQuery {
    agent_id: Option<String>,
    provider_id: Option<String>,
    model: Option<String>,
    mode: Option<String>,
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
    context_cache: Option<ContextCacheUsageResponse>,
    tool_calls: usize,
    checkpoint_count: usize,
    compacted_turns: usize,
    latest_checkpoint_at: Option<String>,
    latest_checkpoint_reason: Option<String>,
    compaction_warning: Option<String>,
    compaction_ratio: f32,
    compaction_reserve_tokens: usize,
    compaction_trigger_tokens: usize,
    compaction_policy_override: bool,
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
struct ContextCacheUsageResponse {
    hit_tokens: u64,
    miss_tokens: u64,
    write_tokens: u64,
    hit_ratio: f32,
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
    let base_config = AppConfig::load_or_default(&state.paths).map_err(WebError::from)?;
    let context_window_tokens =
        usage_context_window(&base_config, &query).map_err(WebError::from)?;
    let config =
        resolve_usage_config(&state.paths, &base_config, &query).map_err(WebError::from)?;
    let mode = AgentMode::parse(query.mode.as_deref()).map_err(WebError::from)?;
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
            last_turn_ttft_ms: 0,
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
    let workspace_path = workspace.path.clone();
    let breakdown =
        match crate::runtime_cwd::scope(std::path::PathBuf::from(&workspace_path), async {
            super::super::services::context_breakdown::estimate_context_breakdown(
                &config,
                &state.paths,
                &store,
                &workspace_path,
                mode,
            )
        })
        .await
        {
            Ok(value) => value,
            Err(_error) => {
                super::super::services::context_breakdown::ContextUsageBreakdown::default()
            }
        };
    // 1. 分项估算合计：无最近一次主对话 provider usage 时用作当前占用
    // 2. 压缩会清空 last_conversation_usage；旧会话若仍残留压缩前 usage，也回退到分项估算
    let breakdown_total = breakdown.total();
    let last_conversation_usage = snapshot.usage.last_conversation_usage.as_ref();
    let last_conversation_tokens =
        last_conversation_usage.map(|usage| usage.prompt_tokens as usize);
    let context_prompt_tokens = resolve_context_prompt_tokens(
        snapshot.context_prompt_tokens,
        last_conversation_tokens,
        breakdown_total,
        snapshot.checkpoint_count,
    );
    let context_cache = build_context_cache_usage(
        last_conversation_usage,
        provider_context_usage_is_current(
            last_conversation_tokens,
            breakdown_total,
            snapshot.checkpoint_count,
        ),
    );
    let context_window_tokens = snapshot.context_window_tokens;
    let context_token_ratio =
        crate::state::context_ratio(context_prompt_tokens, context_window_tokens);
    let resolved = store
        .resolve_compaction_policy(&base_config.context)
        .unwrap_or_else(|_| crate::state::ResolvedCompactionPolicy {
            policy: crate::state::CompactionBudgetPolicy::from_context(
                base_config.context.clamped_compaction_ratio(),
                base_config.context.compaction_reserve_tokens,
            ),
            session_override: false,
        });
    let compaction_trigger_tokens = resolved.policy.trigger_chars(context_window_tokens.max(1));
    Ok(Json(SystemUsageResponse {
        session: SessionUsageResponse {
            id: snapshot.session_id,
            requests: snapshot.usage.requests,
            prompt_tokens: snapshot.usage.prompt_tokens,
            completion_tokens: snapshot.usage.completion_tokens,
            total_tokens: snapshot.usage.total_tokens,
            turn_count: snapshot.turn_count,
            context_prompt_tokens,
            context_window_tokens,
            context_token_ratio,
            context_cache,
            tool_calls: snapshot.tool_history.call_count,
            checkpoint_count: snapshot.checkpoint_count,
            compacted_turns: snapshot.checkpoint_covered_turns,
            latest_checkpoint_at: snapshot.latest_checkpoint_at,
            latest_checkpoint_reason: snapshot.latest_checkpoint_reason,
            compaction_warning: (snapshot.checkpoint_count >= 2).then(|| {
                    "conversation has been compacted multiple times; start a focused session if details become distorted"
                        .to_string()
                }),
            compaction_ratio: resolved.policy.ratio,
            compaction_reserve_tokens: resolved.policy.reserve_tokens,
            compaction_trigger_tokens,
            compaction_policy_override: resolved.session_override,
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

/// 组装当前 Web 请求对应的临时配置。
///
/// 参数:
/// - `paths`: Sai 路径
/// - `config`: 默认配置
/// - `query`: Agent、供应商和模型覆盖
///
/// 返回:
/// - 应用于当前请求的配置
fn resolve_usage_config(
    paths: &crate::paths::SaiPaths,
    config: &AppConfig,
    query: &SystemUsageQuery,
) -> Result<AppConfig> {
    Ok(crate::web::runs::model_override::resolve_run_config(
        paths,
        query.agent_id.as_deref(),
        query.provider_id.as_deref(),
        query.model.as_deref(),
        None,
    )?
    .unwrap_or_else(|| config.clone()))
}

/// 选择用于顶栏展示的当前上下文占用。
///
/// 参数:
/// - `snapshot_tokens`: 会话快照给出的占用
/// - `last_conversation_tokens`: 最近一次主对话 provider prompt_tokens
/// - `breakdown_total`: 实时分项估算合计
/// - `checkpoint_count`: 已应用 checkpoint 数
///
/// 返回:
/// - 展示用上下文 token 数
fn resolve_context_prompt_tokens(
    snapshot_tokens: usize,
    last_conversation_tokens: Option<usize>,
    breakdown_total: usize,
    checkpoint_count: usize,
) -> usize {
    // 1. 无 provider 主对话 usage 时，优先分项估算
    let Some(last_tokens) = last_conversation_tokens.filter(|tokens| *tokens > 0) else {
        return breakdown_total.max(snapshot_tokens);
    };
    // 2. 已压缩且 provider 旧值显著高于实时估算时，判定为压缩前残留
    if provider_context_usage_is_stale(last_tokens, breakdown_total, checkpoint_count) {
        return breakdown_total;
    }
    // 3. 其余情况沿用 provider / 快照值
    last_tokens.max(snapshot_tokens)
}

/// 判断最近一次 provider 上下文用量是否仍可用于当前会话。
///
/// 参数:
/// - `last_conversation_tokens`: 最近一次主对话 provider prompt_tokens
/// - `breakdown_total`: 实时分项估算合计
/// - `checkpoint_count`: 已应用 checkpoint 数
///
/// 返回:
/// - provider 用量存在且不是压缩前残留时返回 true
fn provider_context_usage_is_current(
    last_conversation_tokens: Option<usize>,
    breakdown_total: usize,
    checkpoint_count: usize,
) -> bool {
    let Some(last_tokens) = last_conversation_tokens.filter(|tokens| *tokens > 0) else {
        return false;
    };
    !provider_context_usage_is_stale(last_tokens, breakdown_total, checkpoint_count)
}

/// 判断 provider 上下文用量是否为压缩前残留值。
///
/// 参数:
/// - `last_tokens`: 最近一次主对话 provider prompt_tokens
/// - `breakdown_total`: 实时分项估算合计
/// - `checkpoint_count`: 已应用 checkpoint 数
///
/// 返回:
/// - 已压缩且 provider 值显著高于实时估算时返回 true
fn provider_context_usage_is_stale(
    last_tokens: usize,
    breakdown_total: usize,
    checkpoint_count: usize,
) -> bool {
    checkpoint_count > 0
        && breakdown_total > 0
        && last_tokens > breakdown_total.saturating_mul(3) / 2
}

/// 构造当前上下文的缓存命中统计。
///
/// 参数:
/// - `usage`: 最近一次主对话 provider 用量
/// - `provider_usage_current`: 该用量是否仍代表当前上下文
///
/// 返回:
/// - 缓存命中、未命中、写入量和命中率；无有效 provider 用量时返回 None
fn build_context_cache_usage(
    usage: Option<&Usage>,
    provider_usage_current: bool,
) -> Option<ContextCacheUsageResponse> {
    if !provider_usage_current {
        return None;
    }
    let usage = usage?;
    if usage.prompt_tokens == 0 {
        return None;
    }
    // 1. 异常上报先收敛到 prompt_tokens 范围内
    let hit_tokens = usage.cache_read_tokens.min(usage.prompt_tokens);
    let write_tokens = usage
        .cache_write_tokens
        .min(usage.prompt_tokens.saturating_sub(hit_tokens));
    // 2. 剩余输入视为缓存未命中
    let miss_tokens = usage
        .prompt_tokens
        .saturating_sub(hit_tokens)
        .saturating_sub(write_tokens);
    Some(ContextCacheUsageResponse {
        hit_tokens,
        miss_tokens,
        write_tokens,
        hit_ratio: hit_tokens as f32 / usage.prompt_tokens as f32,
    })
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
            ..SystemUsageQuery::default()
        };

        assert_eq!(usage_context_window(&config, &query).unwrap(), 256_000);
    }

    #[test]
    fn system_usage_rejects_partial_model_selection() {
        let query = SystemUsageQuery {
            provider_id: Some("provider-a".to_string()),
            model: None,
            ..SystemUsageQuery::default()
        };

        let error = usage_context_window(&AppConfig::default(), &query).unwrap_err();

        assert!(error
            .to_string()
            .contains("provider_id and model must be provided together"));
    }

    #[test]
    fn stale_post_compaction_usage_falls_back_to_breakdown() {
        // 压缩后残留 314k，实时分项约 30k，应回退估算
        assert_eq!(
            resolve_context_prompt_tokens(314_900, Some(314_900), 29_700, 1),
            29_700
        );
        // 无 checkpoint 时仍信 provider
        assert_eq!(
            resolve_context_prompt_tokens(40_000, Some(40_000), 29_700, 0),
            40_000
        );
        // 无 usage 时用分项
        assert_eq!(resolve_context_prompt_tokens(0, None, 12_000, 1), 12_000);
    }

    /// 【Web主界面】【上下文缓存】验证缓存率按当前 prompt 总量计算。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 无
    #[test]
    fn context_cache_usage_reports_hit_and_miss() {
        let usage = Usage {
            prompt_tokens: 100,
            completion_tokens: 20,
            total_tokens: 120,
            cache_read_tokens: 80,
            cache_write_tokens: 0,
        };

        let cache = build_context_cache_usage(Some(&usage), true).unwrap();

        assert_eq!(cache.hit_tokens, 80);
        assert_eq!(cache.miss_tokens, 20);
        assert_eq!(cache.write_tokens, 0);
        assert!((cache.hit_ratio - 0.8).abs() < f32::EPSILON);
    }

    /// 【Web主界面】【上下文缓存】验证压缩前残留用量不进入缓存统计。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 无
    #[test]
    fn stale_context_usage_hides_cache_metrics() {
        assert!(!provider_context_usage_is_current(Some(314_900), 29_700, 1));
        assert!(provider_context_usage_is_current(Some(40_000), 29_700, 0));
    }
}
