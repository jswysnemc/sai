use super::context_runtime::{project_context_runtime, ContextRuntimeProjection};
use crate::agent::{build_base_system_prompt, AgentMode};
use crate::cli::build_tool_registry_with_cached_mcp;
use crate::config::AppConfig;
use crate::llm::ToolDefinition;
use crate::paths::SaiPaths;
use crate::state::StateStore;
use crate::token_estimate;
use crate::tools::{self, ToolRegistry};
use anyhow::Result;
use serde::Serialize;

/// 上下文占用分项（与 Web 系统用量浮层图例对应）。
#[derive(Debug, Clone, Serialize, Default)]
pub(crate) struct ContextUsageBreakdown {
    /// 系统提示词（含 epoch baseline 中除技能目录外的部分，以及本轮动态段）
    pub system_prompt_tokens: usize,
    /// 可见工具定义与子智能体相关上下文
    pub tools_and_agents_tokens: usize,
    /// 对话历史与压缩摘要
    pub conversation_tokens: usize,
    /// 连接器及 MCP 工具定义
    pub connectors_and_mcp_tokens: usize,
    /// 技能目录与技能说明
    pub skills_tokens: usize,
}

impl ContextUsageBreakdown {
    /// 返回当前请求已加载上下文的分项合计。
    ///
    /// 返回:
    /// - 不含尚未提交输入的上下文 token 数
    pub(crate) fn total(&self) -> usize {
        self.system_prompt_tokens
            + self.tools_and_agents_tokens
            + self.conversation_tokens
            + self.connectors_and_mcp_tokens
            + self.skills_tokens
    }
}

/// 估算当前会话上下文各分项 token。
///
/// 参数:
/// - `config`: 应用配置（已按当前模型选择解析上下文窗口）
/// - `paths`: Sai 路径
/// - `store`: 当前会话状态仓储
/// - `workspace_path`: 当前工作区路径
/// - `mode`: 当前运行模式
///
/// 返回:
/// - 上下文分项估算
pub(crate) fn estimate_context_breakdown(
    config: &AppConfig,
    paths: &SaiPaths,
    store: &StateStore,
    workspace_path: &str,
    mode: AgentMode,
) -> Result<ContextUsageBreakdown> {
    let dynamic = project_context_runtime(config, paths, store, workspace_path, mode)?;
    estimate_context_breakdown_with_runtime(config, paths, store, mode, &dynamic)
}

/// 使用已生成的动态上下文估算请求分项，避免预览重复执行记忆召回。
///
/// 参数:
/// - `config`: 当前运行配置
/// - `paths`: Sai 路径
/// - `store`: 当前会话状态仓储
/// - `mode`: 当前运行模式
/// - `dynamic`: 已生成的动态上下文投影
///
/// 返回:
/// - 上下文分项估算
pub(super) fn estimate_context_breakdown_with_runtime(
    config: &AppConfig,
    paths: &SaiPaths,
    store: &StateStore,
    mode: AgentMode,
    dynamic: &ContextRuntimeProjection,
) -> Result<ContextUsageBreakdown> {
    // 1. 当前选择与已保存 epoch 一致时复用，否则按下一轮真实配置即时组装
    let tools_enabled = config.tools.enabled && config.active_model_tools_enabled()?;
    let live_baseline = build_base_system_prompt(config, paths, tools_enabled, None)?;
    let baseline = match store.context_epoch_baseline()? {
        Some(baseline) if baseline == live_baseline => baseline,
        _ => live_baseline,
    };
    let (system_core, skills_from_baseline) = split_skills_section(&baseline);

    // 2. 构建缓存 MCP 的工具注册表，避免轮询时触发网络发现
    let mut registry = if config.tools.enabled {
        build_tool_registry_with_cached_mcp(config, paths, mode)?
    } else {
        ToolRegistry::new()
    };

    // 3. 与真实 Web 会话对齐：交互式工具 + agent 过滤 + goal + progressive load
    if config.tools.enabled {
        tools::register_interactive_tools(
            &mut registry,
            config,
            paths,
            store.state_dir().display().to_string(),
            store.session_id().to_string(),
        );
    }
    apply_web_agent_tool_filter(config, &mut registry)?;
    // 渐进加载由当前 Agent 的 deferred_tools 决定，与真实会话保持一致
    let deferred = config.agent_deferred_tools();
    if config.tools.enabled {
        crate::goal::register_tools_for_config(&mut registry, store.goal_file(), config)?;
        if !deferred.is_empty() {
            tools::register_progressive_loader(&mut registry, deferred);
        }
    }

    // 4. 按渐进式可见性选择当前工具定义
    let visible_names = tools::progressive::visible_tool_names(&registry, deferred);
    let definitions = registry.definitions_for_names(&visible_names);

    // 4. 工具定义拆成 MCP 与非 MCP
    let mut tools_json_parts = Vec::new();
    let mut mcp_json_parts = Vec::new();
    for definition in &definitions {
        let serialized = serialize_tool_definition(definition);
        if is_mcp_tool_name(&definition.function.name) {
            mcp_json_parts.push(serialized);
        } else {
            tools_json_parts.push(serialized);
        }
    }

    // 5. 技能：优先用 baseline 中的目录；否则按当前配置重新生成目录估算
    let skills_text = if skills_from_baseline.trim().is_empty() {
        if config.tools.enabled && config.skills.enabled {
            if !deferred.is_empty() {
                tools::skills_catalog_prompt(config, paths).unwrap_or_default()
            } else {
                tools::skills_prompt(config, paths).unwrap_or_default()
            }
        } else {
            String::new()
        }
    } else {
        skills_from_baseline
    };

    // 6. 对话历史：压缩摘要 + 用户/助手/工具消息
    let history = store.project_history(None)?;
    let mut conversation_parts = Vec::new();
    for message in &history.messages {
        if let Ok(serialized) = serde_json::to_string(message) {
            conversation_parts.push(serialized);
        }
    }

    let system_prompt_tokens = estimate_joined(&[
        system_core.as_str(),
        dynamic.runtime_context.as_str(),
        dynamic.memory_index.as_str(),
    ]);
    let tools_and_agents_tokens = estimate_joined(
        &tools_json_parts
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>(),
    );
    // 压缩摘要在真实请求中位于历史前部，只能计入一次
    let mut conversation_refs = Vec::with_capacity(conversation_parts.len() + 1);
    if !dynamic.compaction_summary.trim().is_empty() {
        conversation_refs.push(dynamic.compaction_summary.as_str());
    }
    conversation_refs.extend(conversation_parts.iter().map(String::as_str));
    let conversation_tokens = estimate_joined(&conversation_refs);
    let connectors_and_mcp_tokens = estimate_joined(
        &mcp_json_parts
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>(),
    );
    let skills_tokens = token_estimate::estimate_tokens(&skills_text);

    Ok(ContextUsageBreakdown {
        system_prompt_tokens,
        tools_and_agents_tokens,
        conversation_tokens,
        connectors_and_mcp_tokens,
        skills_tokens,
    })
}

/// 将 baseline 中的技能目录拆出。
///
/// 参数:
/// - `baseline`: Context Epoch baseline 文本
///
/// 返回:
/// - (系统主体, 技能目录片段)
fn split_skills_section(baseline: &str) -> (String, String) {
    const OPEN: &str = "<available-skills>";
    const CLOSE: &str = "</available-skills>";
    let Some(start) = baseline.find(OPEN) else {
        return (baseline.to_string(), String::new());
    };
    let Some(rel_end) = baseline[start..].find(CLOSE) else {
        return (baseline.to_string(), String::new());
    };
    let end = start + rel_end + CLOSE.len();
    let skills = baseline[start..end].to_string();
    let mut system = String::new();
    system.push_str(baseline[..start].trim_end());
    let after = baseline[end..].trim_start();
    if !after.is_empty() {
        if !system.is_empty() {
            system.push_str("\n\n");
        }
        system.push_str(after);
    }
    (system, skills)
}

/// 按 Web 主会话规则过滤 agent 白名单工具。
///
/// 参数:
/// - `config`: 应用配置
/// - `registry`: 待过滤工具注册表
///
/// 返回:
/// - 无
fn apply_web_agent_tool_filter(config: &AppConfig, registry: &mut ToolRegistry) -> Result<()> {
    let Some(runtime) = config.agent_runtime.as_ref() else {
        return Ok(());
    };
    let allowed = runtime
        .enabled_tools
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>();
    let mut filtered = registry.clone_filtered(&allowed);
    if !runtime.exclusive {
        for name in ["subagent", "todo", "ask_question"] {
            if registry.contains(name) {
                filtered.register_from(registry, name)?;
            }
        }
    }
    *registry = filtered;
    Ok(())
}

/// 序列化工具定义为估算用 JSON 文本。
///
/// 参数:
/// - `definition`: 工具定义
///
/// 返回:
/// - JSON 字符串
fn serialize_tool_definition(definition: &ToolDefinition) -> String {
    serde_json::to_string(definition).unwrap_or_else(|_| {
        format!(
            "{}{}{}",
            definition.function.name,
            definition.function.description,
            definition.function.parameters
        )
    })
}

/// 判断是否为 MCP 工具。
///
/// 参数:
/// - `name`: 工具名
///
/// 返回:
/// - 是否 MCP
fn is_mcp_tool_name(name: &str) -> bool {
    name == "mcp_manager" || name.starts_with("mcp_")
}

/// 估算多段文本合计 token。
///
/// 参数:
/// - `parts`: 文本片段
///
/// 返回:
/// - token 数
fn estimate_joined(parts: &[&str]) -> usize {
    let non_empty: Vec<&str> = parts
        .iter()
        .copied()
        .filter(|part| !part.trim().is_empty())
        .collect();
    if non_empty.is_empty() {
        return 0;
    }
    token_estimate::estimate_texts_tokens(&non_empty) as usize
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_skills_section_extracts_catalog() {
        let baseline = "persona\n\n<available-skills>\n- demo\n</available-skills>\n\ntail";
        let (system, skills) = split_skills_section(baseline);
        assert!(system.contains("persona"));
        assert!(system.contains("tail"));
        assert!(!system.contains("available-skills"));
        assert!(skills.contains("<available-skills>"));
        assert!(skills.contains("demo"));
    }

    #[test]
    fn split_skills_section_without_catalog() {
        let (system, skills) = split_skills_section("only system");
        assert_eq!(system, "only system");
        assert!(skills.is_empty());
    }

    /// 验证分项总量不会遗漏任何展示类别。
    #[test]
    fn total_sums_all_categories() {
        let breakdown = ContextUsageBreakdown {
            system_prompt_tokens: 1,
            tools_and_agents_tokens: 2,
            conversation_tokens: 3,
            connectors_and_mcp_tokens: 4,
            skills_tokens: 5,
        };
        assert_eq!(breakdown.total(), 15);
    }
}
