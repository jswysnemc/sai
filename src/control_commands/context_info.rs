use super::context_view::{CategoryUsage, ContextOverview};
use crate::agent::AgentMode;
use crate::config::AppConfig;
use crate::i18n::text as t;
use crate::paths::SaiPaths;
use anyhow::Result;

/// 按当前 REPL 模式组装着色的上下文用量视图。
///
/// 参数:
/// - `paths`: Sai 路径
/// - `mode`: 当前 Agent 模式（影响可见工具与 MCP 估算）
///
/// 返回:
/// - 带 ANSI 的多行文本
#[allow(dead_code)]
pub fn context_info_for_mode(paths: &SaiPaths, mode: AgentMode) -> Result<String> {
    context_info_for(paths, mode, true, None)
}

/// 组装无 ANSI 的上下文用量视图，供网关渠道回复。
///
/// 参数:
/// - `paths`: Sai 路径
///
/// 返回:
/// - 多行纯文本
#[allow(dead_code)]
pub fn context_info_plain(paths: &SaiPaths) -> Result<String> {
    context_info_for(paths, AgentMode::Yolo, false, None)
}

/// 查看上下文用量，并可改写本会话压缩策略。
///
/// 参数:
/// - `paths`: Sai 路径
/// - `mode`: 当前 Agent 模式
/// - `update`: 对本会话策略的改写
///
/// 返回:
/// - 带 ANSI 的多行文本
pub fn context_info_for_mode_with_update(
    paths: &SaiPaths,
    mode: AgentMode,
    update: Option<crate::control_commands::ContextPolicyUpdate>,
) -> Result<String> {
    context_info_for(paths, mode, true, update)
}

/// 网关入口：查看或改写本会话压缩策略。
///
/// 参数:
/// - `paths`: Sai 路径
/// - `update`: 对本会话策略的改写
///
/// 返回:
/// - 多行纯文本
pub fn context_info_plain_with_update(
    paths: &SaiPaths,
    update: Option<crate::control_commands::ContextPolicyUpdate>,
) -> Result<String> {
    context_info_for(paths, AgentMode::Yolo, false, update)
}

/// 【上下文】【视图入口】终端与网关共用数据，终端按实际宽度排版。
/// 参数: 路径、权限模式、着色开关与策略更新；返回: 格式化上下文信息。
fn context_info_for(
    paths: &SaiPaths,
    mode: AgentMode,
    styled: bool,
    update: Option<super::ContextPolicyUpdate>,
) -> Result<String> {
    let view = context_overview(paths, mode, update)?;
    let width = if styled {
        crate::render::fold_text::terminal_wrap_width()
    } else {
        80
    };
    Ok(super::context_view::render(&view, width, styled).join("\n"))
}

/// 【上下文】【数据快照】读取分类估算、真实用量和压缩边界，不执行工具或连接 MCP。
/// 参数: 路径、权限模式与策略更新；返回: 浮层所需的只读数据。
pub(crate) fn context_overview(
    paths: &SaiPaths,
    mode: AgentMode,
    update: Option<super::ContextPolicyUpdate>,
) -> Result<ContextOverview> {
    let config = AppConfig::load_or_default(paths)?;
    if config.agent.engine.is_external() {
        return Ok(ContextOverview {
            external_engine: Some(config.agent.engine.display_label().into()),
            ..Default::default()
        });
    }
    let state = crate::state::StateStore::new(paths)?;
    apply_context_policy_update(&state, &config, update)?;
    let window = config.active_context_window_tokens().unwrap_or(128_000);
    let snapshot = state.session_snapshot(window)?;
    let workspace = crate::runtime_cwd::current_dir()?.display().to_string();
    let breakdown = crate::web::services::context_breakdown::estimate_cached_context_breakdown(
        &config, paths, &state, &workspace, mode,
    )
    .ok();
    let categories = categories(&state, breakdown.as_ref(), snapshot.context_prompt_tokens)?;
    let policy = state.resolve_compaction_policy(&config.context)?.policy;
    let compact_trigger =
        crate::state::CompactionBudgetPolicy::from_context(policy.ratio, policy.reserve_tokens)
            .trigger_chars(window);
    let turn_usage = state.latest_turn_usage()?;
    let usage_is_turn = turn_usage.is_some();
    Ok(ContextOverview {
        used: snapshot.context_prompt_tokens,
        window,
        categories,
        compact_trigger,
        usage: turn_usage.or(snapshot.usage.last_conversation_usage),
        usage_is_turn,
        external_engine: None,
    })
}

/// 【上下文】【分类估算】区分用户、回复、工具参数与工具结果，技能并入系统提示。
/// 参数: 会话仓储、已有分项估算和回退占用；返回: 六类占用数据。
fn categories(
    state: &crate::state::StateStore,
    raw: Option<&crate::web::services::context_breakdown::ContextUsageBreakdown>,
    used: usize,
) -> Result<Vec<CategoryUsage>> {
    let Some(raw) = raw else {
        return Ok(vec![CategoryUsage {
            label: t("Used", "已使用").into(),
            tokens: used,
            color: (255, 214, 165),
        }]);
    };
    let history = state.project_history(None)?;
    let mut user = 0;
    let mut assistant = 0;
    let mut calls = 0;
    let mut results = 0;
    let mut extra_system = 0;
    for message in &history.messages {
        let size = crate::token_estimate::estimate_tokens(&serde_json::to_string(message)?);
        match message.role.as_str() {
            "user" => user += size,
            "tool" => results += size,
            "assistant" => {
                let call_size = message
                    .tool_calls
                    .as_ref()
                    .map(|calls| {
                        serde_json::to_string(calls)
                            .map(|text| crate::token_estimate::estimate_tokens(&text))
                    })
                    .transpose()?
                    .unwrap_or(0)
                    .min(size);
                calls += call_size;
                assistant += size.saturating_sub(call_size);
            }
            _ => extra_system += size,
        }
    }
    let measured = user + assistant + calls + results + extra_system;
    extra_system += raw.conversation_tokens.saturating_sub(measured);
    Ok([
        (
            t("System prompt", "系统提示词"),
            raw.system_prompt_tokens + raw.skills_tokens + extra_system,
            (255, 214, 165),
        ),
        (
            t("Tool definitions", "工具定义"),
            raw.tools_and_agents_tokens + raw.connectors_and_mcp_tokens,
            (205, 165, 168),
        ),
        (t("Your messages", "你的消息"), user, (132, 213, 172)),
        (t("Model replies", "模型回复"), assistant, (211, 131, 135)),
        (t("Tool calls", "工具调用"), calls, (255, 180, 107)),
        (t("Tool results", "工具结果"), results, (207, 170, 133)),
    ]
    .into_iter()
    .map(|(label, tokens, color)| CategoryUsage {
        label: label.into(),
        tokens,
        color,
    })
    .collect())
}

/// 把 `/context` 参数写进当前会话策略。
///
/// 参数:
/// - `state`: 当前会话状态
/// - `config`: 全局配置，供合并未改动的字段
/// - `update`: 策略改写
///
/// 返回:
/// - 写入是否成功
fn apply_context_policy_update(
    state: &crate::state::StateStore,
    config: &AppConfig,
    update: Option<crate::control_commands::ContextPolicyUpdate>,
) -> Result<()> {
    let Some(update) = update else {
        return Ok(());
    };
    match update {
        crate::control_commands::ContextPolicyUpdate::Edit => {
            anyhow::bail!("interactive context editor requires a terminal")
        }
        crate::control_commands::ContextPolicyUpdate::Reset => state.clear_compaction_policy(),
        crate::control_commands::ContextPolicyUpdate::Set {
            ratio_percent,
            reserve,
        } => {
            let current = state.resolve_compaction_policy(&config.context)?;
            let ratio = crate::config::parse_compaction_ratio_value(ratio_percent as f32 / 100.0);
            let reserve_tokens = reserve.unwrap_or(current.policy.reserve_tokens);
            state.save_compaction_policy(ratio, reserve_tokens)
        }
    }
}
