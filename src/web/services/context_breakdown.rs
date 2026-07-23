use crate::agent::AgentMode;
use crate::cli::build_tool_registry_with_cached_mcp;
use crate::config::AppConfig;
use crate::llm::ToolDefinition;
use crate::paths::SaiPaths;
use crate::state::StateStore;
use crate::token_estimate;
use crate::tools::{self, ToolRegistry};
use anyhow::Result;
use serde::Serialize;
use std::collections::BTreeSet;

/// 上下文占用分项（与 Web 系统用量浮层图例对应）。
#[derive(Debug, Clone, Serialize, Default)]
pub(crate) struct ContextUsageBreakdown {
    /// 系统提示词（含 epoch baseline 中除技能目录外的部分，以及模式提醒等）
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

/// 估算当前会话上下文各分项 token。
///
/// 参数:
/// - `config`: 应用配置（已按当前模型选择解析上下文窗口）
/// - `paths`: Sai 路径
/// - `store`: 当前会话状态仓储
///
/// 返回:
/// - 上下文分项估算
pub(crate) fn estimate_context_breakdown(
    config: &AppConfig,
    paths: &SaiPaths,
    store: &StateStore,
) -> Result<ContextUsageBreakdown> {
    // 1. 读取已持久化的系统提示 baseline，并尽量拆出技能目录
    let baseline = store.context_epoch_baseline()?.unwrap_or_default();
    let (system_core, skills_from_baseline) = split_skills_section(&baseline);

    // 2. 构建缓存 MCP 的工具注册表，避免轮询时触发网络发现
    let mut registry = if config.tools.enabled {
        build_tool_registry_with_cached_mcp(config, paths, AgentMode::Yolo)?
    } else {
        ToolRegistry::new()
    };
    apply_web_agent_tool_filter(config, &mut registry)?;

    // 3. 按渐进式可见性选择当前工具定义
    let loaded = store.load_loaded_tools().unwrap_or_default();
    let progressive = config.tools.progressive_loading_enabled;
    let visible_names = visible_tool_names(&registry, progressive, &loaded);
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
            if config.tools.progressive_loading_enabled {
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
    if let Some(context) = history.checkpoint_context.as_ref() {
        conversation_parts.push(context.clone());
    }
    for message in &history.messages {
        if let Ok(serialized) = serde_json::to_string(message) {
            conversation_parts.push(serialized);
        }
    }

    // 7. 已加载工具的动态系统提示（归入工具与子智能体）
    let loaded_prompt = loaded_tools_prompt(progressive, &loaded, &registry);

    // 8. 模式/审计提醒（取最长模式文本作上界）、选中模型标签、运行时上下文
    let mode_reminder = [
        crate::prompts::YOLO_REMINDER,
        crate::prompts::AUDITED_REMINDER,
        crate::prompts::AUTO_AUDIT_REMINDER,
        crate::prompts::PLAN_REMINDER,
    ]
    .into_iter()
    .max_by_key(|text| text.len())
    .unwrap_or("");
    let selected_model = config
        .provider(None)
        .ok()
        .map(|provider| {
            format!(
                "<selected-model>{} / {}</selected-model>",
                provider.display_name, provider.default_model
            )
        })
        .unwrap_or_default();
    let runtime_context = format!(
        "<system-reminder>\n当前系统时间：runtime\n当前工作目录：{}\n</system-reminder>",
        crate::runtime_cwd::current_dir()
            .map(|path| path.display().to_string())
            .unwrap_or_else(|_| "unknown".to_string())
    );
    // 会话标题 / Goal 上下文（若有）
    let goal_context = store
        .goal()
        .ok()
        .flatten()
        .map(|goal| crate::goal::system_context(&goal))
        .unwrap_or_default();
    // 会话标题类：goal 上下文已计入；会话记忆摘要暂并入 conversation 难以拆分时略过正文读取
    let session_memory = String::new();

    let system_prompt_tokens = estimate_joined(&[
        system_core.as_str(),
        mode_reminder,
        selected_model.as_str(),
        runtime_context.as_str(),
        goal_context.as_str(),
        session_memory.as_str(),
    ]);
    let tools_and_agents_tokens = estimate_joined(
        &tools_json_parts
            .iter()
            .map(String::as_str)
            .chain(std::iter::once(loaded_prompt.as_str()))
            .collect::<Vec<_>>(),
    );
    let conversation_tokens = estimate_joined(
        &conversation_parts
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>(),
    );
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
    for name in ["subagent", "todo", "ask_question"] {
        if registry.contains(name) {
            let _ = filtered.register_from(registry, name);
        }
    }
    *registry = filtered;
    Ok(())
}

/// 计算当前应暴露给模型的工具名集合。
///
/// 参数:
/// - `registry`: 完整工具注册表
/// - `progressive`: 是否渐进式加载
/// - `loaded`: 会话已额外加载的工具名
///
/// 返回:
/// - 可见工具名集合
fn visible_tool_names(
    registry: &ToolRegistry,
    progressive: bool,
    loaded: &[String],
) -> BTreeSet<String> {
    let loaded_set: BTreeSet<String> = loaded.iter().cloned().collect();
    registry
        .tool_infos()
        .into_iter()
        .filter(|info| {
            !progressive
                || tools::is_initial_tool(&info.name)
                || loaded_set.contains(&info.name)
        })
        .map(|info| info.name)
        .collect()
}

/// 生成已加载工具的动态系统提示。
///
/// 参数:
/// - `progressive`: 是否渐进式加载
/// - `loaded`: 已加载工具名
/// - `registry`: 工具注册表
///
/// 返回:
/// - 提示文本
fn loaded_tools_prompt(progressive: bool, loaded: &[String], registry: &ToolRegistry) -> String {
    if !progressive || loaded.is_empty() {
        return String::new();
    }
    let names = loaded
        .iter()
        .filter(|name| registry.contains(name))
        .cloned()
        .collect::<Vec<_>>();
    if names.is_empty() {
        return String::new();
    }
    format!(
        "<loaded_tools>\nThe following tools are already loaded in this conversation. Do not call load for them again; call the loaded tool directly. If one of these tools returns an error, treat it as an execution or workflow error, not as a loading error.\nLoaded tools: {}\n</loaded_tools>",
        names.join(", ")
    )
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
            definition.function.name, definition.function.description, definition.function.parameters
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
}
