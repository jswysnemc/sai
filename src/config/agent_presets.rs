use super::agent_tool_modes::{normalize_deferred_tools, DEFERRED_ALL_NON_BASE};
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
    "get_weather",
    "get_exchange_rate",
    "query_deepseek_status",
    "write_memory",
    "read_memory",
    "list_memory",
    "delete_memory",
    "search_evicted_context",
    "archwiki_query",
    "archlinux_official_package_query",
    "aur_search_packages",
    "aur_get_package_info",
    "online_man_search",
    "online_man_get_page",
    "scientific_calculator",
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
    "write_file",
    "str_replace",
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
    "write_memory",
    "read_memory",
    "list_memory",
    "delete_memory",
    "search_evicted_context",
    "search_knowledge_base",
    "search_knowledge_base_by_name",
    "read_knowledge_base_file",
    "upload_text_to_knowledge_base",
    "edit_knowledge_base_file",
    "check_issue",
    "linux_input_method_diagnose",
    "linux_game_compatibility",
    "archwiki_query",
    "archlinux_official_package_query",
    "aur_search_packages",
    "aur_get_package_info",
    "online_man_search",
    "online_man_get_page",
    "review_aur_package",
    "scientific_calculator",
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
    "ask_question",
    "archwiki_query",
    "archlinux_official_package_query",
    "aur_search_packages",
    "aur_get_package_info",
    "online_man_search",
    "online_man_get_page",
    "search_knowledge_base",
    "search_knowledge_base_by_name",
    "read_knowledge_base_file",
    "read_memory",
    "list_memory",
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

/// 探索与 Plan Agent 的检索类工具保持初始可见。
///
/// 这两个 Agent 的核心动作就是检索，把 web_search / web_fetch 推到 load 后面
/// 只会让每次调研都先多一轮加载。
const SEARCH_KEEP_VISIBLE: &[&str] = &["web_search", "web_fetch"];

/// 网关 Agent 需要保持初始可见的工具。
///
/// 渠道消息发送与定时任务是网关的固有职责，延迟加载没有收益。
const GATEWAY_KEEP_VISIBLE: &[&str] = &["send_channel_message", "cron"];

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
    let tools = if !profile.enabled_tools.is_empty() {
        profile.enabled_tools.clone()
    } else {
        match profile.id.as_str() {
            EXPLORE_AGENT_ID => tools_to_owned(EXPLORE_AGENT_TOOLS),
            PLAN_AGENT_ID => tools_to_owned(PLAN_AGENT_TOOLS),
            GATEWAY_AGENT_ID => tools_to_owned(GATEWAY_AGENT_TOOLS),
            GENERAL_AGENT_ID => tools_to_owned(CODE_AGENT_TOOLS),
            _ => Vec::new(),
        }
    };
    expand_legacy_enabled_tools(tools)
}

/// 解析 Agent 档案中需要 load 才暴露的工具列表。
///
/// 档案未显式配置时不做推断：旧配置升级后行为保持不变，
/// 三段状态的默认划分只写在首次落盘的 seed 档案里。
///
/// 参数:
/// - `profile`: Agent 档案
/// - `enabled`: 已解析的启用工具白名单
///
/// 返回:
/// - 收敛到白名单内的延迟工具名
pub(super) fn resolve_deferred_tools(profile: &AgentProfile, enabled: &[String]) -> Vec<String> {
    normalize_deferred_tools(enabled, &profile.deferred_tools)
}

/// 从白名单中划出默认需要 load 的工具。
///
/// 基础文件、命令与任务工具保持会话开始即可见；其余按用途分组的工具
/// 交给模型按需 load，避免首轮就把全部工具描述塞进上下文。
///
/// 参数:
/// - `tools`: Agent 工具白名单
/// - `keep_visible`: 即使不属于基础工具也要保持初始可见的工具名
///
/// 返回:
/// - 默认延迟加载的工具名
fn deferred_from_whitelist(tools: &[&str], keep_visible: &[&str]) -> Vec<String> {
    tools
        .iter()
        .filter(|name| !crate::tools::groups::is_base_tool(name))
        .filter(|name| !keep_visible.contains(*name))
        .map(|name| (*name).to_string())
        .collect()
}

/// 将旧工具名展开为当前注册名，并补齐编辑工具组合。
///
/// 参数:
/// - `tools`: 配置中的白名单
///
/// 返回:
/// - 与当前 ToolRegistry 名称对齐后的白名单
fn expand_legacy_enabled_tools(mut tools: Vec<String>) -> Vec<String> {
    if tools.is_empty() {
        return tools;
    }
    let has = |tools: &[String], name: &str| tools.iter().any(|tool| tool == name);
    let push_if_missing = |tools: &mut Vec<String>, name: &str| {
        if !tools.iter().any(|tool| tool == name) {
            tools.push(name.to_string());
        }
    };
    // 1. 旧局部替换工具映射到 str_replace
    if has(&tools, "replace_file_lines") {
        push_if_missing(&mut tools, "str_replace");
    }
    // 2. 旧 apply_patch / edit_file 统一映射到 str_replace：
    //    Codex patch 工具已移除，局部编辑与整文件写入分别由 str_replace 和 write_file 承担
    if has(&tools, "apply_patch") || has(&tools, "edit_file") {
        push_if_missing(&mut tools, "str_replace");
    }
    // 3. 具备 write_file 的工程 Agent 默认补上 str_replace
    if has(&tools, "write_file") {
        push_if_missing(&mut tools, "str_replace");
    }
    // 4. 网页读取、天气、汇率、DeepSeek、手册与计算器旧名映射到当前注册名
    if has(&tools, "fetch_url") {
        push_if_missing(&mut tools, "web_fetch");
    }
    if has(&tools, "query_weather") {
        push_if_missing(&mut tools, "get_weather");
    }
    if has(&tools, "convert_exchange_rate") || has(&tools, "exchange_rate") {
        push_if_missing(&mut tools, "get_exchange_rate");
    }
    if has(&tools, "deepseek_status") {
        push_if_missing(&mut tools, "query_deepseek_status");
    }
    if has(&tools, "man_page_search") || has(&tools, "man_search") {
        push_if_missing(&mut tools, "online_man_search");
    }
    if has(&tools, "man_page_read") || has(&tools, "man_read") {
        push_if_missing(&mut tools, "online_man_get_page");
    }
    if has(&tools, "calculate") || has(&tools, "calculator") {
        push_if_missing(&mut tools, "scientific_calculator");
    }
    tools
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
        // 全量开放且无法穷举工具名，用通配符表达「基础工具直接可见，其余按需 load」
        deferred_tools: vec![DEFERRED_ALL_NON_BASE.to_string()],
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
        deferred_tools: deferred_from_whitelist(CODE_AGENT_TOOLS, SEARCH_KEEP_VISIBLE),
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
        deferred_tools: deferred_from_whitelist(EXPLORE_AGENT_TOOLS, SEARCH_KEEP_VISIBLE),
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
        deferred_tools: deferred_from_whitelist(PLAN_AGENT_TOOLS, SEARCH_KEEP_VISIBLE),
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
        deferred_tools: deferred_from_whitelist(GATEWAY_AGENT_TOOLS, GATEWAY_KEEP_VISIBLE),
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
