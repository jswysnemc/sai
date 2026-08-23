use super::agents::SubagentConfig;
use super::cli_tools::PluginsConfig;
use super::git::{GitConfig, ScmConfig};
use super::model::*;
use super::permission::PermissionConfig;
use super::session::SessionConfig;
use crate::default_models::OPENCODE_PROVIDER_ID;

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            prompt_sections: Default::default(),
            active_provider: OPENCODE_PROVIDER_ID.to_string(),
            providers: ProviderConfig::default_templates(),
            agent: crate::config::AgentEngineConfig::default(),
            permission: PermissionConfig::default(),
            session: SessionConfig::default(),
            notification: crate::config::NotificationConfig::default(),
            context: ContextConfig::default(),
            tools: ToolsConfig::default(),
            terminal: TerminalConfig::default(),
            skills: SkillsConfig::default(),
            display: DisplayConfig::default(),
            debug: DebugConfig::default(),
            scm: ScmConfig::default(),
            git: GitConfig::default(),
            ssh: super::ssh::SshConfig::default(),
            prompt: PromptConfig::default(),
            gateways: GatewayConfig::default(),
            agents: Vec::new(),
            // 首次 init 会 seed agents；此处给出入口默认 id
            default_agent: Some("general".to_string()),
            tui_agent: Some("general".to_string()),
            cli_agent: Some("cli".to_string()),
            gateway_agent: Some("gateway".to_string()),
            subagent: SubagentConfig::default(),
            agent_runtime: None,
            hooks: HooksConfig::default(),
            mcp: McpConfig::default(),
            plugins: PluginsConfig::default(),
            memory: MemoryConfig::default(),
            system_prompt_file: Some("system-prompt.md".to_string()),
            system_prompt: None,
            load_instruction_files: true,
        }
    }
}

impl Default for DebugConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            retain_logs: true,
        }
    }
}

impl Default for PromptConfig {
    fn default() -> Self {
        Self {
            identities_dir: default_identities_dir(),
            user_identity_file: default_user_identity_file(),
            active_identity: String::new(),
            templates: super::PromptTemplatesConfig::default(),
        }
    }
}

impl Default for DisplayConfig {
    fn default() -> Self {
        Self {
            reasoning: default_reasoning_display(),
            tool_calls: default_tool_call_display(),
            readable_tool_names: default_true(),
            wait_show_model: default_true(),
            wait_show_thinking_level: default_true(),
            repl_transcript_row_cap: default_repl_transcript_row_cap(),
        }
    }
}

impl Default for ToolsConfig {
    fn default() -> Self {
        Self {
            enabled: default_true(),
            max_rounds: 0,
            command_shell: String::new(),
            command_filter: default_command_filter(),
            command_filter_denylist: Vec::new(),
            background_commands_enabled: default_true(),
            background_command_timeout_seconds: default_background_command_timeout_seconds(),
            background_command_log_max_bytes: default_background_command_log_max_bytes(),
            background_command_stop_grace_seconds: default_background_command_stop_grace_seconds(),
        }
    }
}

impl Default for TerminalConfig {
    fn default() -> Self {
        Self {
            shell: default_terminal_shell(),
        }
    }
}

/// 返回网页终端的默认 Shell 配置值。
///
/// 返回:
/// - Unix 用户环境中的 Shell；Windows PowerShell
fn default_terminal_shell() -> String {
    #[cfg(windows)]
    {
        let system_root = std::env::var_os("SystemRoot").unwrap_or_else(|| "C:\\Windows".into());
        let powershell = std::path::PathBuf::from(system_root)
            .join("System32")
            .join("WindowsPowerShell")
            .join("v1.0")
            .join("powershell.exe");
        if powershell.is_file() {
            return powershell.to_string_lossy().into_owned();
        }
        "powershell.exe".to_string()
    }
    #[cfg(not(windows))]
    {
        std::env::var("SHELL")
            .ok()
            .filter(|shell| !shell.trim().is_empty())
            .filter(|shell| std::path::Path::new(shell).is_file())
            .unwrap_or_default()
    }
}

impl Default for SkillsConfig {
    fn default() -> Self {
        Self {
            enabled: default_true(),
            allow_command_execution: default_true(),
        }
    }
}

impl Default for MemoryConfig {
    fn default() -> Self {
        Self {
            enabled: default_true(),
            evicted_context_enabled: default_true(),
            association_enabled: default_true(),
            snippet_chars: default_memory_snippet_chars(),
            extraction_provider_id: String::new(),
            extraction_model: String::new(),
        }
    }
}

impl Default for ContextConfig {
    fn default() -> Self {
        Self {
            default_max_chars: default_context_chars(),
            compaction_ratio: default_compaction_ratio(),
            compaction_reserve_tokens: default_compaction_reserve_tokens(),
            compaction_provider_id: String::new(),
            compaction_model: String::new(),
        }
    }
}

pub(super) fn default_timeout() -> u64 {
    60
}

/// 命令输出过滤器默认档位：探测到 rtk 时自动启用。
pub(super) fn default_command_filter() -> String {
    "auto".to_string()
}

pub(super) fn default_background_command_timeout_seconds() -> u64 {
    0
}

pub(super) fn default_background_command_log_max_bytes() -> u64 {
    10 * 1024 * 1024
}

pub(super) fn default_background_command_stop_grace_seconds() -> u64 {
    5
}

pub(super) fn default_identities_dir() -> String {
    "identities".to_string()
}

pub(super) fn default_user_identity_file() -> String {
    "user-identity.md".to_string()
}

pub(super) fn default_qq_gateway_listen() -> String {
    "127.0.0.1:8766".to_string()
}

pub(super) fn default_qq_gateway_transport() -> String {
    "websocket".to_string()
}

pub(super) fn default_qq_gateway_base_url() -> String {
    "https://api.sgroup.qq.com".to_string()
}

/// 飞书事件订阅默认监听地址。
pub(super) fn default_feishu_gateway_listen() -> String {
    "127.0.0.1:8790".to_string()
}

/// 飞书开放平台默认接口地址。
pub(super) fn default_feishu_gateway_base_url() -> String {
    "https://open.feishu.cn".to_string()
}

pub(super) fn default_weixin_gateway_base_url() -> String {
    "https://ilinkai.weixin.qq.com".to_string()
}

pub(super) fn default_weixin_gateway_cdn_base_url() -> String {
    "https://novac2c.cdn.weixin.qq.com/c2c".to_string()
}

pub(super) fn default_weixin_gateway_bot_type() -> String {
    "3".to_string()
}

pub(super) fn default_temperature() -> f32 {
    0.7
}

pub(super) fn default_anthropic_max_tokens() -> u32 {
    4096
}

pub(super) fn default_thinking_level() -> String {
    "auto".to_string()
}

pub(super) fn default_thinking_format() -> String {
    "auto".to_string()
}

pub(super) fn is_default_timeout(value: &u64) -> bool {
    *value == default_timeout()
}

/// 判断子智能体配置是否为默认空值,用于序列化时跳过。
///
/// 参数:
/// - `value`: 子智能体配置
///
/// 返回:
/// - 是否为默认空配置
pub(super) fn is_default_subagent(value: &super::agents::SubagentConfig) -> bool {
    value.provider_id.is_empty()
        && value.model.is_empty()
        && (value.thinking_level.is_empty() || value.thinking_level == "auto")
        && value.default_profile.is_empty()
        && value.profiles.is_empty()
}

pub(super) fn is_default_temperature(value: &f32) -> bool {
    (*value - default_temperature()).abs() < f32::EPSILON
}

pub(super) fn is_default_anthropic_max_tokens(value: &u32) -> bool {
    *value == default_anthropic_max_tokens()
}

pub(super) fn is_auto_thinking_level(value: &str) -> bool {
    value.trim().is_empty() || value == "auto"
}

pub(super) fn is_auto_thinking_format(value: &str) -> bool {
    value.trim().is_empty() || value == "auto"
}

pub(super) fn default_provider_protocol() -> String {
    "auto".to_string()
}

pub(super) fn is_auto_protocol(value: &str) -> bool {
    value.trim().is_empty() || value == "auto"
}

pub(super) fn default_true() -> bool {
    true
}

/// 判断历史 `tools.max_rounds` 是否为零。
///
/// 参数:
/// - `value`: 配置值
///
/// 返回:
/// - 为零时返回 true，序列化时省略该键
pub(super) fn is_zero_max_rounds(value: &usize) -> bool {
    *value == 0
}

/// 是否为默认的启用状态（序列化时可省略）。
///
/// 参数:
/// - `value`: 当前开关值
///
/// 返回:
/// - 与默认值一致时返回 true
pub(super) fn is_default_true(value: &bool) -> bool {
    *value
}

pub(super) fn default_reasoning_display() -> String {
    "summary".to_string()
}

pub(super) fn default_tool_call_display() -> String {
    "summary".to_string()
}

pub(super) fn default_repl_transcript_row_cap() -> usize {
    5_000
}

pub(super) fn default_memory_snippet_chars() -> usize {
    500
}

pub(super) fn default_print_image_width_percent() -> u8 {
    45
}

pub(super) fn default_print_image_height_percent() -> u8 {
    35
}

pub(super) fn default_memes_width_percent() -> u8 {
    35
}

pub(super) fn default_memes_height_percent() -> u8 {
    25
}

pub(super) fn default_memes_max_image_mb() -> u64 {
    10
}

pub(super) fn default_memes_auto_send_probability() -> f32 {
    0.2
}

pub(super) fn default_memes_auto_send_min_confidence() -> f32 {
    0.8
}

/// 返回 Web 搜索默认供应商路由模式。
pub(super) fn default_web_search_provider() -> String {
    "auto".to_string()
}

/// 返回 Web 搜索默认结果数量。
pub(super) fn default_web_search_max_results() -> usize {
    5
}

/// 返回 Web 搜索默认请求超时秒数。
pub(super) fn default_web_search_timeout() -> u64 {
    20
}

/// 返回 TinyFish 官方默认接口地址。
pub(super) fn default_tinyfish_base_url() -> String {
    "https://api.search.tinyfish.ai".to_string()
}

/// 返回 Tavily 官方默认搜索接口地址。
pub(super) fn default_tavily_base_url() -> String {
    "https://api.tavily.com/search".to_string()
}

/// 返回 Tavily 默认搜索深度。
pub(super) fn default_tavily_search_depth() -> String {
    "basic".to_string()
}

/// 返回 Firecrawl 官方默认搜索接口地址。
pub(super) fn default_firecrawl_base_url() -> String {
    "https://api.firecrawl.dev/v2/search".to_string()
}

/// 返回 AnySearch 官方默认搜索接口地址。
pub(super) fn default_anysearch_base_url() -> String {
    "https://api.anysearch.com/v1/search".to_string()
}

/// 返回 SearXNG 默认语言模式。
pub(super) fn default_searxng_language() -> String {
    "auto".to_string()
}

pub(super) fn default_web_images_max_results() -> usize {
    5
}

pub(super) fn default_web_images_max_download_mb() -> f64 {
    4.0
}

pub(super) fn default_web_images_preview_count() -> usize {
    1
}

pub(super) fn default_web_images_timeout() -> u64 {
    20
}

pub(super) fn default_deep_diagnose_depth() -> String {
    "high".to_string()
}

pub(super) fn default_deep_diagnose_max_review_revisions() -> usize {
    0
}

pub(super) fn default_deep_diagnose_max_tool_steps() -> usize {
    0
}

pub(super) fn default_deep_diagnose_tool_timeout() -> u64 {
    90
}

pub(super) fn default_subagent_max_tool_steps() -> usize {
    100
}

pub(super) fn default_image_generation_provider_type() -> String {
    "openai".to_string()
}

pub(super) fn default_openai_images_base_url() -> String {
    "https://api.openai.com".to_string()
}

pub(super) fn default_image_generation_model() -> String {
    "gpt-image-1".to_string()
}

pub(super) fn default_image_generation_aspect_ratio() -> String {
    "自动".to_string()
}

pub(super) fn default_image_generation_resolution() -> String {
    "1K".to_string()
}

pub(super) fn default_image_generation_output_dir() -> String {
    if let Some(dirs) = directories::UserDirs::new() {
        if let Some(pictures) = dirs.picture_dir() {
            return pictures.join("sai/generated-images").display().to_string();
        }
    }
    "~/Pictures/sai/generated-images".to_string()
}

pub(super) fn default_image_generation_timeout() -> u64 {
    180
}

pub(super) fn default_kb_max_search_results() -> usize {
    5
}

pub(super) fn default_kb_snippet_context_chars() -> usize {
    240
}

pub(super) fn default_kb_proximity_window_chars() -> usize {
    512
}

pub(super) fn default_kb_max_read_lines() -> usize {
    200
}

pub(super) fn default_kb_max_file_size_kb() -> usize {
    1024
}

pub(super) fn default_kb_allowed_extensions() -> String {
    ".txt,.md,.json,.jsonc,.json5,.yaml,.yml,.csv,.log,.py,.js,.ts,.jsx,.tsx,.mjs,.cjs,.html,.css,.scss,.sass,.less,.cfg,.ini,.conf,.toml,.kdl,.desktop,.service,.timer,.socket,.target,.mount,.rules,.network,.netdev,.properties,.hjson,.ron,.rst,.xml,.sh,.bash,.zsh,.fish,.nu,.ps1,.lua,.nix,.rasi,.yuck,.sql,.rs,.go,.c,.h,.cpp,.hpp,.java,.kt,.php,.rb,.pl,.org,.adoc,.tex".to_string()
}

pub(super) fn default_kb_allowed_filenames() -> String {
    ".env,.env.local,.env.example,.env.sample,.envrc,.editorconfig,.gitignore,.gitattributes,.npmrc,.vimrc,.bashrc,.zshrc,.profile,.xinitrc,.xresources,config,dockerfile,containerfile,makefile,justfile,procfile,pkgbuild".to_string()
}

pub(super) fn default_kb_semantic_chunk_chars() -> usize {
    512
}

pub(super) fn default_kb_semantic_chunk_overlap() -> usize {
    80
}

pub(super) fn default_kb_semantic_top_k() -> usize {
    5
}

pub(super) fn default_kb_semantic_min_score() -> f32 {
    0.25
}

pub(super) fn default_kb_keyword_strong_score_threshold() -> f32 {
    180.0
}

pub(super) fn default_kb_embedding_timeout_seconds() -> u64 {
    60
}

pub(super) fn default_diagnostics_timeout() -> u64 {
    5
}

pub(super) fn default_diagnostics_max_stdout_chars() -> usize {
    8_000
}

pub(super) fn default_diagnostics_max_stderr_chars() -> usize {
    4_000
}

pub(super) fn default_calculator_backend() -> String {
    "rust-simple".to_string()
}

pub(super) fn default_context_chars() -> usize {
    120_000
}

pub(super) fn default_compaction_ratio() -> f32 {
    0.9
}

pub(super) fn default_compaction_reserve_tokens() -> usize {
    50_000
}

pub(super) fn is_default_compaction_ratio(value: &f32) -> bool {
    (*value - default_compaction_ratio()).abs() < f32::EPSILON
}

pub(super) fn is_default_compaction_reserve_tokens(value: &usize) -> bool {
    *value == default_compaction_reserve_tokens()
}

/// 默认客户端模拟风格。
/// Codex CLI 默认 User-Agent（与探测报告 0.144.0 对齐）。
pub(crate) const CODEX_CLI_USER_AGENT: &str = "codex_cli_rs/0.144.0";

/// Claude Code CLI 默认 User-Agent（与抓包 2.1.113 对齐）。
pub(crate) const CLAUDE_CLI_USER_AGENT: &str = "claude-cli/2.1.113 (external, cli)";

/// 非 Codex / Claude 模式下的默认 HTTP User-Agent。
pub(crate) const DEFAULT_HTTP_USER_AGENT: &str = "sai/0.1";

pub(super) fn default_client_style() -> String {
    "auto".to_string()
}

/// 是否为 auto 客户端风格（序列化时可省略）。
pub(super) fn is_auto_client_style(value: &str) -> bool {
    value.trim().is_empty() || value.eq_ignore_ascii_case("auto")
}

/// Claude Code 模拟默认启用 1M 上下文 beta。
pub(super) fn default_claude_1m_context() -> bool {
    true
}

/// 是否为默认 1M 上下文开关（序列化时可省略）。
pub(super) fn is_default_claude_1m_context(value: &bool) -> bool {
    *value == default_claude_1m_context()
}
