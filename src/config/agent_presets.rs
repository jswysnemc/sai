use super::agents::{
    AgentProfile, CLI_AGENT_ID, EXPLORE_AGENT_ID, GATEWAY_AGENT_ID, GENERAL_AGENT_ID, PLAN_AGENT_ID,
};

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

/// 返回所有内置 Agent 档案。
///
/// 参数:
/// - 无
///
/// 返回:
/// - CLI、代码、探索、Plan 与网关档案
pub(super) fn builtin_agent_profiles() -> [AgentProfile; 5] {
    [
        builtin_cli_agent(),
        builtin_general_agent(),
        builtin_explore_agent(),
        builtin_plan_agent(),
        builtin_gateway_agent(),
    ]
}

/// 解析 Agent 档案的工具列表。
///
/// 参数:
/// - `profile`: Agent 档案
///
/// 返回:
/// - 空向量表示全量工具；非空为白名单
pub(super) fn resolve_enabled_tools(profile: &AgentProfile) -> Vec<String> {
    if !profile.enabled_tools.is_empty() {
        return profile.enabled_tools.clone();
    }
    match profile.id.as_str() {
        EXPLORE_AGENT_ID => tools_to_owned(EXPLORE_AGENT_TOOLS),
        PLAN_AGENT_ID => tools_to_owned(PLAN_AGENT_TOOLS),
        GATEWAY_AGENT_ID => tools_to_owned(GATEWAY_AGENT_TOOLS),
        GENERAL_AGENT_ID => tools_to_owned(CODE_AGENT_TOOLS),
        _ => Vec::new(),
    }
}

/// 将静态工具名称转换为配置持有的字符串。
///
/// 参数:
/// - `tools`: 静态工具名称列表
///
/// 返回:
/// - 可写入 Agent 档案的工具名称列表
fn tools_to_owned(tools: &[&str]) -> Vec<String> {
    tools.iter().map(|tool| (*tool).to_string()).collect()
}

/// 构造 CLI 终端助手默认档案。
///
/// 参数:
/// - 无
///
/// 返回:
/// - 全量开放工具的 CLI 档案
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
///
/// 参数:
/// - 无
///
/// 返回:
/// - 适用于长程编程任务的代码 Agent 档案
fn builtin_general_agent() -> AgentProfile {
    AgentProfile {
        id: GENERAL_AGENT_ID.to_string(),
        name: "代码 Agent".to_string(),
        description: "适合实现、测试、文档和常规工程任务；工具面向长程编程".to_string(),
        system_prompt: GENERAL_AGENT_PROMPT.to_string(),
        enabled_tools: tools_to_owned(CODE_AGENT_TOOLS),
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
        enabled_tools: tools_to_owned(EXPLORE_AGENT_TOOLS),
        thinking_level: "auto".to_string(),
        register_to_main: true,
        load_instruction_files: true,
        ..AgentProfile::default()
    }
}

/// 构造只读 Plan Agent。
///
/// 参数:
/// - 无
///
/// 返回:
/// - 限制为只读工具的 Plan Agent 档案
fn builtin_plan_agent() -> AgentProfile {
    AgentProfile {
        id: PLAN_AGENT_ID.to_string(),
        name: "Plan Agent".to_string(),
        description: "只读调研与方案规划，不改系统状态".to_string(),
        system_prompt: PLAN_AGENT_PROMPT.to_string(),
        enabled_tools: tools_to_owned(PLAN_AGENT_TOOLS),
        thinking_level: "auto".to_string(),
        register_to_main: true,
        load_instruction_files: true,
        ..AgentProfile::default()
    }
}

/// 构造网关内置 Agent 档案。
///
/// 参数:
/// - 无
///
/// 返回:
/// - 适用于即时通讯网关的 Agent 档案
fn builtin_gateway_agent() -> AgentProfile {
    AgentProfile {
        id: GATEWAY_AGENT_ID.to_string(),
        name: "网关 Agent".to_string(),
        description: "适合 QQ/微信等即时通讯网关：短回复、排障与查询".to_string(),
        system_prompt: GATEWAY_AGENT_PROMPT.to_string(),
        enabled_tools: tools_to_owned(GATEWAY_AGENT_TOOLS),
        thinking_level: "auto".to_string(),
        register_to_main: false,
        load_instruction_files: false,
        ..AgentProfile::default()
    }
}

/// 首次运行写入配置文件的默认 Agent 列表。
///
/// 参数:
/// - 无
///
/// 返回:
/// - CLI、代码、探索、Plan 与网关档案
pub fn seed_default_agent_profiles() -> Vec<AgentProfile> {
    builtin_agent_profiles().into_iter().collect()
}

/// 为尚未指定入口默认 Agent 的配置补齐入口默认值。
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
