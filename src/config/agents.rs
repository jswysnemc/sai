use serde::{Deserialize, Serialize};

pub const DEFAULT_AGENT_ID: &str = "default";
pub const CLI_AGENT_ID: &str = "cli";
pub const GENERAL_AGENT_ID: &str = "general";
pub const EXPLORE_AGENT_ID: &str = "explore";
pub const PLAN_AGENT_ID: &str = "plan";
pub const GATEWAY_AGENT_ID: &str = "gateway";

const CLI_AGENT_PROMPT: &str = include_str!("../prompts/cli-agent.md");
const GENERAL_AGENT_PROMPT: &str = include_str!("../prompts/code-agent.md");
const EXPLORE_AGENT_PROMPT: &str = include_str!("../prompts/explore-agent.md");
const PLAN_AGENT_PROMPT: &str = include_str!("../prompts/plan-agent.md");
const GATEWAY_AGENT_PROMPT: &str = include_str!("../prompts/gateway-agent.md");

const GATEWAY_AGENT_TOOLS: &[&str] = &[
    "check_os_info",
    "read_file",
    "glob",
    "grep",
    "run_command",
    "web_search",
    "web_fetch",
    "query_weather",
    "get_weather",
    "convert_exchange_rate",
    "deepseek_status",
    "remember_fact",
    "recall_memories",
    "recall_past_events",
    "search_evicted_context",
    "archwiki_query",
    "archlinux_official_package_query",
    "aur_search_packages",
    "aur_get_package_info",
    "man_page_search",
    "man_page_read",
    "calculate",
    "calculate_hash",
    "decode_encoded_text",
    "set_alarm",
    "list_alarms",
    "cancel_alarm",
    "search_knowledge_base",
    "read_knowledge_base_file",
    "search_knowledge_base_by_name",
    "cron",
    "send_channel_message",
];


/// TUI / Web 长程编程默认工具白名单（排除表情包、趣味占卜等助手娱乐工具）。
const CODE_AGENT_TOOLS: &[&str] = &[
    "run_command",
    "background_command",
    "subagent",
    "todo",
    "edit_file",
    "apply_patch",
    "write_file",
    "replace_file_lines",
    "create_goal",
    "get_goal",
    "update_goal",
    "trash_path",
    "check_os_info",
    "read_file",
    "glob",
    "grep",
    "ask_question",
    "web_search",
    "web_fetch",
    "fetch_url",
    "remember_fact",
    "recall_memories",
    "recall_past_events",
    "search_evicted_context",
    "search_knowledge_base",
    "search_knowledge_base_by_name",
    "read_knowledge_base_file",
    "upload_text_to_knowledge_base",
    "edit_knowledge_base_file",
    "deep_research",
    "check_issue",
    "linux_input_method_diagnose",
    "linux_game_compatibility",
    "archwiki_query",
    "archlinux_official_package_query",
    "aur_search_packages",
    "aur_get_package_info",
    "man_page_search",
    "man_page_read",
    "review_aur_package",
    "calculate",
    "calculate_hash",
    "decode_encoded_text",
    "mcp_manager",
];

/// Plan Agent 只读工具。
const PLAN_AGENT_TOOLS: &[&str] = &[
    "check_os_info",
    "read_file",
    "glob",
    "grep",
    "web_search",
    "web_fetch",
    "fetch_url",
    "ask_question",
    "archwiki_query",
    "archlinux_official_package_query",
    "aur_search_packages",
    "aur_get_package_info",
    "man_page_search",
    "man_page_read",
    "search_knowledge_base",
    "search_knowledge_base_by_name",
    "read_knowledge_base_file",
    "recall_memories",
    "recall_past_events",
    "search_evicted_context",
];

const EXPLORE_AGENT_TOOLS: &[&str] = &[
    "check_os_info",
    "read_file",
    "glob",
    "grep",
    "web_search",
    "web_fetch",
];

/// 选择默认 Agent 的运行入口。
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum AgentSurface {
    Web,
    Tui,
    Cli,
    Gateway,
}

/// 仅在单轮运行期间生效的 Agent 能力覆盖。
#[derive(Debug, Clone, PartialEq)]
pub struct AgentRuntimeOverride {
    /// 允许使用的工具名称
    pub enabled_tools: Vec<String>,
    /// 完整暴露的 skills
    pub skills_full: Vec<String>,
    /// 仅暴露名称的 skills
    pub skills_named: Vec<String>,
}

/// Agent 配置档案。
///
/// 描述一个可复用的 Agent 预设：运行模型、系统提示词、能力集合和注册范围。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AgentProfile {
    /// Agent 唯一标识
    pub id: String,
    /// Agent 显示名称
    pub name: String,
    /// 主 Agent 选择或委派时展示的用途描述
    #[serde(default)]
    pub description: String,
    /// 系统提示词全文
    #[serde(default)]
    pub system_prompt: String,
    /// 启用的工具，可填写工具名或工具分组名
    #[serde(default)]
    pub enabled_tools: Vec<String>,
    /// 完整启用的 skills：加载名称与描述
    #[serde(default)]
    pub skills_full: Vec<String>,
    /// 半启用的 skills：仅暴露名称
    #[serde(default)]
    pub skills_named: Vec<String>,
    /// 可选供应商 id，空表示沿用当前供应商
    #[serde(default)]
    pub provider_id: String,
    /// 可选模型，空表示沿用供应商当前模型
    #[serde(default)]
    pub model: String,
    /// 可选思考等级，auto 表示沿用当前配置
    #[serde(default = "default_agent_thinking_level")]
    pub thinking_level: String,
    /// 是否向主 Agent 注册为可调用的子 Agent
    #[serde(default)]
    pub register_to_main: bool,
    /// 是否加载全局 / 项目 AGENT.md、AGENTS.md、CLAUDE.md 等指令文件
    #[serde(default = "default_true")]
    pub load_instruction_files: bool,
}

/// 旧版可由主 Agent 选择的子 Agent 档案，仅用于配置兼容迁移。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SubagentProfile {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub system_prompt: String,
    #[serde(default)]
    pub provider_id: String,
    #[serde(default)]
    pub model: String,
    #[serde(default = "default_agent_thinking_level")]
    pub thinking_level: String,
    #[serde(default = "default_true")]
    pub exposed: bool,
}

/// 旧版子智能体运行配置。
///
/// 新配置应改用统一 AgentProfile；这些字段继续支持已有配置。
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct SubagentConfig {
    /// 子智能体使用的供应商 id，空表示沿用主对话
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub provider_id: String,
    /// 子智能体使用的模型，空表示沿用该供应商默认模型
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub model: String,
    #[serde(default = "default_agent_thinking_level")]
    pub thinking_level: String,
    #[serde(default)]
    pub default_profile: String,
    #[serde(default)]
    pub profiles: Vec<SubagentProfile>,
}

impl Default for AgentProfile {
    fn default() -> Self {
        Self {
            id: String::new(),
            name: String::new(),
            description: String::new(),
            system_prompt: String::new(),
            enabled_tools: Vec::new(),
            skills_full: Vec::new(),
            skills_named: Vec::new(),
            provider_id: String::new(),
            model: String::new(),
            thinking_level: default_agent_thinking_level(),
            register_to_main: false,
            load_instruction_files: true,
        }
    }
}

impl AgentProfile {
    /// 将旧子 Agent 档案转换为统一 Agent 档案。
    ///
    /// 参数:
    /// - `profile`: 旧子 Agent 档案
    ///
    /// 返回:
    /// - 可用于统一运行时的 Agent 档案
    fn from_legacy_subagent(profile: SubagentProfile) -> Self {
        Self {
            id: profile.id,
            name: profile.name,
            description: profile.description,
            system_prompt: profile.system_prompt,
            enabled_tools: Vec::new(),
            skills_full: Vec::new(),
            skills_named: Vec::new(),
            provider_id: profile.provider_id,
            model: profile.model,
            thinking_level: profile.thinking_level,
            register_to_main: profile.exposed,
            load_instruction_files: true,
        }
    }
}

impl crate::config::AppConfig {
    /// 返回包含内置通用、探索和旧配置迁移结果的统一 Agent 列表。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 去重后的 Agent 档案
    pub fn resolved_agent_profiles(&self) -> Vec<AgentProfile> {
        let legacy = &self.subagent.profiles;
        let mut profiles = [
            builtin_cli_agent(),
            builtin_general_agent(),
            builtin_explore_agent(),
            builtin_plan_agent(),
            builtin_gateway_agent(),
        ]
        .into_iter()
        .map(|builtin| {
            self.agents
                .iter()
                .find(|profile| profile.id == builtin.id)
                .cloned()
                .or_else(|| {
                    legacy
                        .iter()
                        .find(|profile| profile.id == builtin.id)
                        .cloned()
                        .map(AgentProfile::from_legacy_subagent)
                })
                .unwrap_or(builtin)
        })
        .collect::<Vec<_>>();
        for legacy in legacy.iter().cloned() {
            if profiles.iter().any(|profile| profile.id == legacy.id)
                || self.agents.iter().any(|profile| profile.id == legacy.id)
            {
                continue;
            }
            profiles.push(AgentProfile::from_legacy_subagent(legacy));
        }
        profiles.extend(
            self.agents
                .iter()
                .filter(|profile| {
                    !matches!(
                        profile.id.as_str(),
                        CLI_AGENT_ID
                            | GENERAL_AGENT_ID
                            | EXPLORE_AGENT_ID
                            | PLAN_AGENT_ID
                            | GATEWAY_AGENT_ID
                    )
                })
                .cloned(),
        );
        profiles
    }

    /// 解析指定入口默认使用的 Agent 标识。
    ///
    /// 参数:
    /// - `surface`: 当前运行入口
    ///
    /// 返回:
    /// - 配置的 Agent 标识
    pub fn default_agent_for_surface(&self, surface: AgentSurface) -> Option<&str> {
        let value = match surface {
            AgentSurface::Web => self.default_agent.as_deref(),
            AgentSurface::Tui => self.tui_agent.as_deref(),
            AgentSurface::Cli => self.cli_agent.as_deref(),
            AgentSurface::Gateway => self.gateway_agent.as_deref(),
        };
        value.map(str::trim).filter(|value| !value.is_empty())
    }

    /// 解析主 Agent 可调用的已注册 Agent。
    ///
    /// 参数:
    /// - `requested`: 主 Agent 显式选择的 Agent 标识
    ///
    /// 返回:
    /// - 已注册的 Agent 档案
    pub fn resolve_registered_agent(&self, requested: Option<&str>) -> Option<AgentProfile> {
        let requested = requested
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .or_else(|| {
                (!self.subagent.default_profile.trim().is_empty())
                    .then_some(self.subagent.default_profile.trim())
            })
            .unwrap_or(GENERAL_AGENT_ID);
        self.resolved_agent_profiles()
            .into_iter()
            .find(|profile| profile.register_to_main && profile.id == requested)
    }
}

/// 把指定 Agent 档案应用到运行期配置。
///
/// 参数:
/// - `config`: 当前应用配置
/// - `agent_id`: 调用方显式选择的 Agent 标识
/// - `surface`: 当前运行入口
///
/// 返回:
/// - 已应用模型、提示词和能力覆盖的配置
pub fn apply_agent_override(
    mut config: crate::config::AppConfig,
    agent_id: Option<&str>,
    surface: AgentSurface,
) -> anyhow::Result<crate::config::AppConfig> {
    use anyhow::bail;

    // 1. 显式选择优先，未指定时采用当前入口默认值
    let explicit = agent_id.map(str::trim).filter(|value| !value.is_empty());
    let selected = explicit.map(str::to_string).or_else(|| {
        config
            .default_agent_for_surface(surface)
            .map(str::to_string)
    });
    let Some(agent_id) = selected else {
        return Ok(config);
    };
    // 2. 从统一列表解析内置、旧版迁移或自定义档案
    let profile = config
        .resolved_agent_profiles()
        .into_iter()
        .find(|profile| profile.id == agent_id);
    let Some(profile) = profile else {
        if agent_id == DEFAULT_AGENT_ID {
            return Ok(config);
        }
        bail!("agent not found: {agent_id}");
    };
    // 3. 应用提示词、供应商、模型和思考等级覆盖
    if !profile.system_prompt.trim().is_empty() {
        config.system_prompt_file = None;
        config.system_prompt = Some(profile.system_prompt.clone());
    }
    if !profile.provider_id.trim().is_empty() {
        config.active_provider = profile.provider_id.clone();
    }
    if let Some(provider) = config
        .providers
        .iter_mut()
        .find(|provider| provider.id == config.active_provider)
    {
        if !profile.model.trim().is_empty() {
            provider.default_model = profile.model.clone();
        }
        if !profile.thinking_level.trim().is_empty() && profile.thinking_level != "auto" {
            provider.thinking_level = profile.thinking_level.clone();
        }
    }
    // 4. 工具白名单：空列表表示全量；内置 explore/plan/gateway/code 有默认白名单
    let enabled_tools = resolve_enabled_tools(&profile);
    config.load_instruction_files = profile.load_instruction_files;
    config.agent_runtime = if enabled_tools.is_empty()
        && profile.skills_full.is_empty()
        && profile.skills_named.is_empty()
    {
        None
    } else {
        Some(AgentRuntimeOverride {
            enabled_tools,
            skills_full: profile.skills_full,
            skills_named: profile.skills_named,
        })
    };
    Ok(config)
}

/// 解析 Agent 档案的工具列表。
///
/// 参数:
/// - `profile`: Agent 档案
///
/// 返回:
/// - 空向量表示全量工具；非空为白名单
fn resolve_enabled_tools(profile: &AgentProfile) -> Vec<String> {
    if !profile.enabled_tools.is_empty() {
        return profile.enabled_tools.clone();
    }
    match profile.id.as_str() {
        EXPLORE_AGENT_ID => EXPLORE_AGENT_TOOLS
            .iter()
            .map(|tool| (*tool).to_string())
            .collect(),
        PLAN_AGENT_ID => PLAN_AGENT_TOOLS
            .iter()
            .map(|tool| (*tool).to_string())
            .collect(),
        GATEWAY_AGENT_ID => GATEWAY_AGENT_TOOLS
            .iter()
            .map(|tool| (*tool).to_string())
            .collect(),
        GENERAL_AGENT_ID => CODE_AGENT_TOOLS
            .iter()
            .map(|tool| (*tool).to_string())
            .collect(),
        _ => Vec::new(),
    }
}

/// 构造 CLI 终端助手默认档案（全量工具 + 人格提示）。
fn builtin_cli_agent() -> AgentProfile {
    AgentProfile {
        id: CLI_AGENT_ID.to_string(),
        name: "CLI 助手".to_string(),
        description: "人格化终端助手：工具全量开放，适合日常排障与对话".to_string(),
        system_prompt: CLI_AGENT_PROMPT.to_string(),
        enabled_tools: Vec::new(),
        thinking_level: "auto".to_string(),
        register_to_main: false,
        load_instruction_files: true,
        ..AgentProfile::default()
    }
}

/// 构造 TUI / Web 长程代码 Agent 档案。
fn builtin_general_agent() -> AgentProfile {
    AgentProfile {
        id: GENERAL_AGENT_ID.to_string(),
        name: "代码 Agent".to_string(),
        description: "适合实现、测试、文档和常规工程任务；工具面向长程编程".to_string(),
        system_prompt: GENERAL_AGENT_PROMPT.to_string(),
        enabled_tools: CODE_AGENT_TOOLS
            .iter()
            .map(|tool| (*tool).to_string())
            .collect(),
        thinking_level: "auto".to_string(),
        register_to_main: true,
        load_instruction_files: true,
        ..AgentProfile::default()
    }
}

/// 构造可由用户覆盖的内置探索 Agent 档案。
///
/// 参数:
/// - 无
///
/// 返回:
/// - 限制为只读检索工具的探索档案
fn builtin_explore_agent() -> AgentProfile {
    AgentProfile {
        id: EXPLORE_AGENT_ID.to_string(),
        name: "探索 Agent".to_string(),
        description: "适合只读检索、代码定位和资料探索；返回证据与路径".to_string(),
        system_prompt: EXPLORE_AGENT_PROMPT.to_string(),
        enabled_tools: EXPLORE_AGENT_TOOLS
            .iter()
            .map(|tool| (*tool).to_string())
            .collect(),
        thinking_level: "auto".to_string(),
        register_to_main: true,
        load_instruction_files: true,
        ..AgentProfile::default()
    }
}

/// 构造只读 Plan Agent。
fn builtin_plan_agent() -> AgentProfile {
    AgentProfile {
        id: PLAN_AGENT_ID.to_string(),
        name: "Plan Agent".to_string(),
        description: "只读调研与方案规划，不改系统状态".to_string(),
        system_prompt: PLAN_AGENT_PROMPT.to_string(),
        enabled_tools: PLAN_AGENT_TOOLS
            .iter()
            .map(|tool| (*tool).to_string())
            .collect(),
        thinking_level: "auto".to_string(),
        register_to_main: true,
        load_instruction_files: true,
        ..AgentProfile::default()
    }
}

/// 构造网关（微信/QQ 等）内置 Agent。
fn builtin_gateway_agent() -> AgentProfile {
    AgentProfile {
        id: GATEWAY_AGENT_ID.to_string(),
        name: "网关 Agent".to_string(),
        description: "适合 QQ/微信等即时通讯网关：短回复、排障与查询".to_string(),
        system_prompt: GATEWAY_AGENT_PROMPT.to_string(),
        enabled_tools: GATEWAY_AGENT_TOOLS
            .iter()
            .map(|tool| (*tool).to_string())
            .collect(),
        thinking_level: "auto".to_string(),
        register_to_main: false,
        load_instruction_files: false,
        ..AgentProfile::default()
    }
}

fn default_agent_thinking_level() -> String {
    "auto".to_string()
}

fn default_true() -> bool {
    true
}

/// 首次运行写入配置文件的默认 Agent 列表。
///
/// 返回:
/// - CLI / 代码 / 探索 / Plan / 网关档案
pub fn seed_default_agent_profiles() -> Vec<AgentProfile> {
    vec![
        builtin_cli_agent(),
        builtin_general_agent(),
        builtin_explore_agent(),
        builtin_plan_agent(),
        builtin_gateway_agent(),
    ]
}

/// 为尚未指定入口默认 Agent 的配置补齐表面默认值。
///
/// 参数:
/// - `config`: 待补齐配置
///
/// 返回:
/// - 是否改动了配置
pub fn ensure_surface_agent_defaults(config: &mut crate::config::AppConfig) -> bool {
    let mut changed = false;
    if config
        .cli_agent
        .as_deref()
        .map(str::trim)
        .unwrap_or("")
        .is_empty()
    {
        config.cli_agent = Some(CLI_AGENT_ID.to_string());
        changed = true;
    }
    if config
        .tui_agent
        .as_deref()
        .map(str::trim)
        .unwrap_or("")
        .is_empty()
    {
        config.tui_agent = Some(GENERAL_AGENT_ID.to_string());
        changed = true;
    }
    if config
        .default_agent
        .as_deref()
        .map(str::trim)
        .unwrap_or("")
        .is_empty()
    {
        config.default_agent = Some(GENERAL_AGENT_ID.to_string());
        changed = true;
    }
    if config
        .gateway_agent
        .as_deref()
        .map(str::trim)
        .unwrap_or("")
        .is_empty()
    {
        config.gateway_agent = Some(GATEWAY_AGENT_ID.to_string());
        changed = true;
    }
    if config.agents.is_empty() {
        config.agents = seed_default_agent_profiles();
        changed = true;
    }
    changed
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 验证统一 Agent 配置可以覆盖内置探索 Agent 并关闭主 Agent 注册。
    #[test]
    fn unified_agents_override_builtin_registration() {
        let mut config = crate::config::AppConfig::default();
        config.agents.push(AgentProfile {
            id: EXPLORE_AGENT_ID.to_string(),
            name: "项目探索".to_string(),
            description: "只查项目".to_string(),
            register_to_main: false,
            ..AgentProfile::default()
        });

        assert!(config
            .resolved_agent_profiles()
            .iter()
            .any(|profile| profile.id == EXPLORE_AGENT_ID && profile.name == "项目探索"));
        assert!(config
            .resolve_registered_agent(Some(EXPLORE_AGENT_ID))
            .is_none());
    }

    /// 验证 CLI 与 TUI 可以选择不同的默认 Agent。
    #[test]
    fn applies_surface_specific_default_agents() {
        let mut config = crate::config::AppConfig::default();
        config.agents.push(AgentProfile {
            id: "cli-agent".to_string(),
            name: "CLI".to_string(),
            system_prompt: "cli prompt".to_string(),
            ..AgentProfile::default()
        });
        config.agents.push(AgentProfile {
            id: "tui-agent".to_string(),
            name: "TUI".to_string(),
            system_prompt: "tui prompt".to_string(),
            ..AgentProfile::default()
        });
        config.cli_agent = Some("cli-agent".to_string());
        config.tui_agent = Some("tui-agent".to_string());

        let cli = apply_agent_override(config.clone(), None, AgentSurface::Cli).unwrap();
        let tui = apply_agent_override(config, None, AgentSurface::Tui).unwrap();
        assert_eq!(cli.system_prompt.as_deref(), Some("cli prompt"));
        assert_eq!(tui.system_prompt.as_deref(), Some("tui prompt"));
    }

    /// 内置代码 Agent 带有工程规范提示词；探索 Agent 为只读。
    #[test]
    fn builtin_agents_include_default_prompts() {
        let cli = builtin_cli_agent();
        let general = builtin_general_agent();
        let explore = builtin_explore_agent();
        let plan = builtin_plan_agent();
        assert!(cli.system_prompt.contains("Sai"));
        assert!(cli.enabled_tools.is_empty());
        assert!(general.system_prompt.contains("核心铁律"));
        assert!(!general.enabled_tools.is_empty());
        assert!(explore.system_prompt.contains("只读"));
        assert!(!explore.enabled_tools.is_empty());
        assert!(plan.system_prompt.contains("Plan"));
        assert!(!plan.enabled_tools.is_empty());
    }

    /// 默认入口：CLI 助手全量；TUI/Web 代码 Agent；网关专用。
    #[test]
    fn default_surfaces_use_cli_and_code_agents() {
        let mut config = crate::config::AppConfig::default();
        ensure_surface_agent_defaults(&mut config);
        assert_eq!(config.cli_agent.as_deref(), Some(CLI_AGENT_ID));
        assert_eq!(config.tui_agent.as_deref(), Some(GENERAL_AGENT_ID));
        assert_eq!(config.default_agent.as_deref(), Some(GENERAL_AGENT_ID));
        assert_eq!(config.gateway_agent.as_deref(), Some(GATEWAY_AGENT_ID));
        let cli = apply_agent_override(config.clone(), None, AgentSurface::Cli).unwrap();
        assert!(cli.agent_runtime.is_none(), "CLI 应继承全量工具");
        assert!(cli.system_prompt.as_deref().unwrap_or("").contains("Sai"));
        assert!(cli.load_instruction_files);
        let tui = apply_agent_override(config.clone(), None, AgentSurface::Tui).unwrap();
        let runtime = tui.agent_runtime.expect("code agent whitelist");
        assert!(runtime.enabled_tools.iter().any(|t| t == "edit_file"));
        assert!(!runtime.enabled_tools.iter().any(|t| t == "show_meme"));
        assert!(tui.system_prompt.as_deref().unwrap_or("").contains("核心铁律"));
        let gateway = apply_agent_override(config, None, AgentSurface::Gateway).unwrap();
        assert!(!gateway.load_instruction_files);
        assert!(gateway.agent_runtime.is_some());
    }

    #[test]
    fn explore_and_plan_are_readonly_scoped() {
        let config = crate::config::AppConfig::default();
        let explore =
            apply_agent_override(config.clone(), Some(EXPLORE_AGENT_ID), AgentSurface::Web)
                .unwrap();
        let tools = explore.agent_runtime.unwrap().enabled_tools;
        assert!(tools.iter().any(|t| t == "read_file"));
        assert!(!tools.iter().any(|t| t == "edit_file"));
        let plan = apply_agent_override(config, Some(PLAN_AGENT_ID), AgentSurface::Web).unwrap();
        let tools = plan.agent_runtime.unwrap().enabled_tools;
        assert!(tools.iter().any(|t| t == "web_search"));
        assert!(!tools.iter().any(|t| t == "run_command"));
        assert!(plan.system_prompt.as_deref().unwrap_or("").contains("Plan"));
    }


    /// 验证旧子 Agent 档案会进入统一 Agent 列表并保留暴露状态。
    #[test]
    fn migrates_legacy_subagent_profiles_into_unified_agents() {
        let mut config = crate::config::AppConfig::default();
        config.subagent.profiles = vec![SubagentProfile {
            id: EXPLORE_AGENT_ID.to_string(),
            name: "旧探索".to_string(),
            description: "旧用途".to_string(),
            system_prompt: "旧提示".to_string(),
            provider_id: String::new(),
            model: String::new(),
            thinking_level: "high".to_string(),
            exposed: false,
        }];

        let profile = config
            .resolved_agent_profiles()
            .into_iter()
            .find(|profile| profile.id == EXPLORE_AGENT_ID)
            .unwrap();
        assert_eq!(profile.name, "旧探索");
        assert_eq!(profile.thinking_level, "high");
        assert!(!profile.register_to_main);
    }
}
