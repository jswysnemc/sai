use super::agents::SubagentConfig;
use super::git::{GitConfig, ScmConfig};
use super::model::*;
use super::paths::persona_scope_name;
use super::permission::PermissionConfig;
use crate::default_models::OPENCODE_PROVIDER_ID;
use std::collections::HashMap;

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            active_provider: OPENCODE_PROVIDER_ID.to_string(),
            providers: ProviderConfig::default_templates(),
            permission: PermissionConfig::default(),
            context: ContextConfig::default(),
            tools: ToolsConfig::default(),
            terminal: TerminalConfig::default(),
            skills: SkillsConfig::default(),
            display: DisplayConfig::default(),
            scm: ScmConfig::default(),
            git: GitConfig::default(),
            prompt: PromptConfig::default(),
            gateways: GatewayConfig::default(),
            agents: Vec::new(),
            default_agent: Some("general".to_string()),
            tui_agent: Some("general".to_string()),
            cli_agent: None,
            gateway_agent: Some("gateway".to_string()),
            subagent: SubagentConfig::default(),
            agent_runtime: None,
            hooks: HooksConfig::default(),
            mcp: McpConfig::default(),
            plugins: PluginsConfig::default(),
            memory: MemoryConfig::default(),
            system_prompt_file: Some("system-prompt.md".to_string()),
            system_prompt: None,
        }
    }
}

impl Default for PromptConfig {
    fn default() -> Self {
        Self {
            prompts_dir: default_prompts_dir(),
            identities_dir: default_identities_dir(),
            user_identity_file: default_user_identity_file(),
            active_persona: String::new(),
            active_identity: String::new(),
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

impl Default for PluginsConfig {
    fn default() -> Self {
        Self {
            weather: PluginEnabledConfig::default(),
            web: WebPluginConfig::default(),
            web_images: WebImagesPluginConfig::default(),
            deep_research: DeepResearchPluginConfig::default(),
            deep_diagnose: DeepDiagnosePluginConfig::default(),
            vision: VisionPluginConfig::default(),
            exchange_rate: ExchangeRatePluginConfig::default(),
            xuanxue: PluginEnabledConfig::default(),
            image_generation: ImageGenerationPluginConfig::default(),
            print_image: PrintImagePluginConfig::default(),
            memes: MemesPluginConfig::default(),
            knowledge_base: KnowledgeBasePluginConfig::default(),
            archlinux: PluginEnabledConfig::default(),
            man: PluginEnabledConfig::default(),
            moegirl: PluginEnabledConfig::default(),
            hash_codec: PluginEnabledConfig::default(),
            calculator: CalculatorPluginConfig::default(),
            package_advisor: PluginEnabledConfig::default(),
            linux_game_compatibility: LinuxGameCompatibilityConfig::default(),
            diagnostics: DiagnosticsPluginConfig::default(),
            memory: MemoryConfig::default(),
        }
    }
}

impl Default for PluginEnabledConfig {
    fn default() -> Self {
        Self {
            enabled: default_true(),
        }
    }
}

impl Default for LinuxGameCompatibilityConfig {
    fn default() -> Self {
        Self {
            enabled: default_true(),
            max_tool_steps: default_subagent_max_tool_steps(),
        }
    }
}

impl Default for WebPluginConfig {
    fn default() -> Self {
        Self {
            enabled: default_true(),
            tinyfish_api_keys: Vec::new(),
            tavily_api_keys: Vec::new(),
            firecrawl_api_keys: Vec::new(),
            anysearch_api_keys: Vec::new(),
            searxng_base_url: String::new(),
        }
    }
}

impl Default for WebImagesPluginConfig {
    fn default() -> Self {
        Self {
            enabled: default_true(),
            max_results: default_web_images_max_results(),
            max_download_mb: default_web_images_max_download_mb(),
            safe_search: default_true(),
            vision_screening_enabled: default_true(),
            auto_preview: default_true(),
            preview_count: default_web_images_preview_count(),
            timeout_seconds: default_web_images_timeout(),
        }
    }
}

impl Default for DeepResearchPluginConfig {
    fn default() -> Self {
        Self {
            enabled: default_true(),
            output_dir: default_deep_research_dir(),
            thinking_depth: default_deep_research_depth(),
            max_review_revisions: default_deep_research_max_review_revisions(),
            max_tool_steps_per_round: default_deep_research_max_tool_steps(),
            max_final_answer_chars: 0,
            tool_call_timeout_seconds: default_deep_research_tool_timeout(),
            show_progress: default_true(),
        }
    }
}

impl Default for DeepDiagnosePluginConfig {
    fn default() -> Self {
        Self {
            enabled: default_true(),
            thinking_depth: default_deep_research_depth(),
            max_review_revisions: default_deep_research_max_review_revisions(),
            max_tool_steps_per_round: default_deep_research_max_tool_steps(),
            max_final_answer_chars: 0,
            tool_call_timeout_seconds: default_deep_research_tool_timeout(),
            max_tool_steps: default_subagent_max_tool_steps(),
            show_progress: default_true(),
        }
    }
}

impl Default for VisionPluginConfig {
    fn default() -> Self {
        Self {
            enabled: default_true(),
            prefer_current_multimodal_model: default_true(),
            vision_provider_id: String::new(),
            vision_model: String::new(),
            preview_with_chafa: default_true(),
        }
    }
}

impl Default for ExchangeRatePluginConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            api_key: String::new(),
            free_fallback_enabled: default_true(),
        }
    }
}

impl Default for ImageGenerationPluginConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            provider_type: default_image_generation_provider_type(),
            base_url: default_openai_images_base_url(),
            api_keys: Vec::new(),
            model: default_image_generation_model(),
            default_aspect_ratio: default_image_generation_aspect_ratio(),
            default_resolution: default_image_generation_resolution(),
            output_dir: default_image_generation_output_dir(),
            auto_print: default_true(),
            timeout_seconds: default_image_generation_timeout(),
        }
    }
}

impl Default for PrintImagePluginConfig {
    fn default() -> Self {
        Self {
            enabled: default_true(),
            width_percent: default_print_image_width_percent(),
            height_percent: default_print_image_height_percent(),
        }
    }
}

impl Default for MemesPluginConfig {
    fn default() -> Self {
        Self {
            enabled: default_true(),
            persona_libraries: HashMap::new(),
            width_percent: default_memes_width_percent(),
            height_percent: default_memes_height_percent(),
            max_image_mb: default_memes_max_image_mb(),
            allow_gif_animation: false,
            auto_send_enabled: true,
            auto_send_probability: default_memes_auto_send_probability(),
            auto_send_min_confidence: default_memes_auto_send_min_confidence(),
        }
    }
}

impl MemesPluginConfig {
    pub fn library_for_persona(&self, persona: &str) -> String {
        if persona.trim().is_empty() {
            return self
                .persona_libraries
                .get("default")
                .cloned()
                .unwrap_or_else(|| "sai".to_string());
        }
        let persona = persona_scope_name(persona);
        self.persona_libraries
            .get(&persona)
            .cloned()
            .unwrap_or(persona)
    }
}

impl Default for KnowledgeBasePluginConfig {
    fn default() -> Self {
        Self {
            enabled: default_true(),
            data_dir: String::new(),
            max_search_results: default_kb_max_search_results(),
            snippet_context_chars: default_kb_snippet_context_chars(),
            proximity_window_chars: default_kb_proximity_window_chars(),
            max_read_lines: default_kb_max_read_lines(),
            max_file_size_kb: default_kb_max_file_size_kb(),
            allowed_extensions: default_kb_allowed_extensions(),
            allowed_filenames: default_kb_allowed_filenames(),
            upload_tool_enabled: default_true(),
            embedding_enabled: false,
            embedding_provider_id: String::new(),
            embedding_model: String::new(),
            semantic_chunk_chars: default_kb_semantic_chunk_chars(),
            semantic_chunk_overlap: default_kb_semantic_chunk_overlap(),
            semantic_top_k: default_kb_semantic_top_k(),
            semantic_min_score: default_kb_semantic_min_score(),
            keyword_strong_score_threshold: default_kb_keyword_strong_score_threshold(),
            embedding_timeout_seconds: default_kb_embedding_timeout_seconds(),
        }
    }
}

impl Default for CalculatorPluginConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            backend: default_calculator_backend(),
        }
    }
}

impl Default for DiagnosticsPluginConfig {
    fn default() -> Self {
        Self {
            enabled: default_true(),
            command_timeout_seconds: default_diagnostics_timeout(),
            max_stdout_chars: default_diagnostics_max_stdout_chars(),
            max_stderr_chars: default_diagnostics_max_stderr_chars(),
        }
    }
}

impl Default for ToolsConfig {
    fn default() -> Self {
        Self {
            enabled: default_true(),
            max_rounds: 0,
            command_shell: String::new(),
            progressive_loading_enabled: false,
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
            auto_diary_enabled: default_true(),
            auto_fact_enabled: default_true(),
            auto_skill_enabled: false,
            association_facts: default_memory_association_facts(),
            association_episodes: default_memory_association_episodes(),
            association_max_chars: default_memory_association_max_chars(),
            snippet_chars: default_memory_snippet_chars(),
            forget_after_days: default_memory_forget_after_days(),
            forgetting_enabled: default_true(),
            forgetting_half_life_days: default_memory_half_life_days(),
            forgetting_min_strength: default_memory_min_strength(),
            forgetting_review_boost: default_memory_review_boost(),
            learning_min_task_chars: default_memory_min_task_chars(),
            learning_min_method_chars: default_memory_min_method_chars(),
        }
    }
}

impl Default for ContextConfig {
    fn default() -> Self {
        Self {
            default_max_chars: default_context_chars(),
            compaction_provider_id: String::new(),
            compaction_model: String::new(),
        }
    }
}

pub(super) fn default_timeout() -> u64 {
    60
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

pub(super) fn default_prompts_dir() -> String {
    "prompts".to_string()
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

pub(super) fn default_reasoning_display() -> String {
    "summary".to_string()
}

pub(super) fn default_tool_call_display() -> String {
    "summary".to_string()
}

pub(super) fn default_repl_transcript_row_cap() -> usize {
    5_000
}

pub(super) fn default_memory_association_facts() -> usize {
    5
}

pub(super) fn default_memory_association_episodes() -> usize {
    3
}

pub(super) fn default_memory_association_max_chars() -> usize {
    1800
}

pub(super) fn default_memory_snippet_chars() -> usize {
    500
}

pub(super) fn default_memory_forget_after_days() -> u64 {
    90
}

pub(super) fn default_memory_half_life_days() -> f64 {
    7.0
}

pub(super) fn default_memory_min_strength() -> f64 {
    0.15
}

pub(super) fn default_memory_review_boost() -> f64 {
    0.35
}

pub(super) fn default_memory_min_task_chars() -> usize {
    16
}

pub(super) fn default_memory_min_method_chars() -> usize {
    120
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

pub(super) fn default_deep_research_dir() -> String {
    if let Some(dirs) = directories::UserDirs::new() {
        if let Some(documents) = dirs.document_dir() {
            return documents.join("Sai/deep-thinking").display().to_string();
        }
    }
    "~/Documents/Sai/deep-thinking".to_string()
}

pub(super) fn default_deep_research_depth() -> String {
    "high".to_string()
}

pub(super) fn default_deep_research_max_review_revisions() -> usize {
    0
}

pub(super) fn default_deep_research_max_tool_steps() -> usize {
    0
}

pub(super) fn default_deep_research_tool_timeout() -> u64 {
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
