use super::agents::{AgentProfile, AgentRuntimeOverride, SubagentConfig};
use super::defaults::*;
use super::model_metadata::ModelMetadata;
use super::permission::PermissionConfig;
use serde::{Deserialize, Deserializer, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub active_provider: String,
    pub providers: Vec<ProviderConfig>,
    #[serde(default)]
    pub permission: PermissionConfig,
    #[serde(default)]
    pub context: ContextConfig,
    #[serde(default)]
    pub tools: ToolsConfig,
    #[serde(default)]
    pub terminal: TerminalConfig,
    #[serde(default)]
    pub skills: SkillsConfig,
    #[serde(default)]
    pub display: DisplayConfig,
    #[serde(default)]
    pub prompt: PromptConfig,
    #[serde(default)]
    pub gateways: GatewayConfig,
    /// Agent 配置档案列表，各入口可按档案覆盖模型、提示词、工具和 Skills
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub agents: Vec<AgentProfile>,
    /// Web 默认 Agent 档案 id，未指定 agent_id 的网页运行采用它
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_agent: Option<String>,
    /// TUI REPL 默认使用的 Agent 档案 id
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tui_agent: Option<String>,
    /// 单次 CLI 命令默认使用的 Agent 档案 id
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cli_agent: Option<String>,
    /// 网关（QQ/微信等）默认使用的 Agent 档案 id
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gateway_agent: Option<String>,
    /// 旧版子智能体运行配置，保留用于兼容迁移
    #[serde(default, skip_serializing_if = "is_default_subagent")]
    pub subagent: SubagentConfig,
    /// 单轮运行时 Agent 覆盖，不参与配置序列化
    #[serde(skip)]
    pub agent_runtime: Option<AgentRuntimeOverride>,
    #[serde(default)]
    pub hooks: HooksConfig,
    #[serde(default)]
    pub mcp: McpConfig,
    #[serde(default)]
    pub plugins: PluginsConfig,
    #[serde(default, skip_serializing)]
    pub memory: MemoryConfig,
    #[serde(default)]
    pub system_prompt_file: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub system_prompt: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct DisplayConfig {
    #[serde(default = "default_reasoning_display")]
    pub reasoning: String,
    #[serde(default = "default_tool_call_display")]
    pub tool_calls: String,
    #[serde(default = "default_true")]
    pub readable_tool_names: bool,
    #[serde(default = "default_true")]
    pub wait_show_model: bool,
    #[serde(default = "default_true")]
    pub wait_show_thinking_level: bool,
    #[serde(default = "default_repl_transcript_row_cap")]
    pub repl_transcript_row_cap: usize,
}

#[derive(Debug, Clone, Deserialize)]
struct RawDisplayConfig {
    #[serde(default)]
    reasoning: Option<String>,
    #[serde(default)]
    tool_calls: Option<String>,
    #[serde(default)]
    show_reasoning: Option<bool>,
    #[serde(default)]
    reasoning_mode: Option<String>,
    #[serde(default)]
    show_tool_details: Option<bool>,
    #[serde(default)]
    readable_tool_names: Option<bool>,
    #[serde(default)]
    wait_show_model: Option<bool>,
    #[serde(default)]
    wait_show_thinking_level: Option<bool>,
    #[serde(default)]
    repl_transcript_row_cap: Option<usize>,
}

impl<'de> Deserialize<'de> for DisplayConfig {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let raw = RawDisplayConfig::deserialize(deserializer)?;
        let reasoning = raw.reasoning.unwrap_or_else(|| {
            if raw.show_reasoning == Some(false) {
                "hidden".to_string()
            } else {
                raw.reasoning_mode.unwrap_or_else(default_reasoning_display)
            }
        });
        let tool_calls = raw.tool_calls.unwrap_or_else(|| {
            if raw.show_tool_details == Some(true) {
                "full".to_string()
            } else {
                default_tool_call_display()
            }
        });
        Ok(Self {
            reasoning,
            tool_calls,
            readable_tool_names: raw.readable_tool_names.unwrap_or_else(default_true),
            wait_show_model: raw.wait_show_model.unwrap_or_else(default_true),
            wait_show_thinking_level: raw.wait_show_thinking_level.unwrap_or_else(default_true),
            repl_transcript_row_cap: raw
                .repl_transcript_row_cap
                .unwrap_or_else(default_repl_transcript_row_cap),
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderConfig {
    pub id: String,
    pub display_name: String,
    pub base_url: String,
    #[serde(
        default = "default_provider_protocol",
        skip_serializing_if = "is_auto_protocol"
    )]
    pub protocol: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_key: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub models: Vec<String>,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub model_context_chars: HashMap<String, usize>,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub model_metadata: HashMap<String, ModelMetadata>,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub default_model: String,
    #[serde(
        default = "default_timeout",
        skip_serializing_if = "is_default_timeout"
    )]
    pub timeout_seconds: u64,
    #[serde(
        default = "default_temperature",
        skip_serializing_if = "is_default_temperature"
    )]
    pub temperature: f32,
    /// Anthropic Messages API 的 max_tokens（仅 anthropic 协议使用）。
    #[serde(
        default = "default_anthropic_max_tokens",
        skip_serializing_if = "is_default_anthropic_max_tokens"
    )]
    pub anthropic_max_tokens: u32,
    #[serde(
        default = "default_thinking_level",
        skip_serializing_if = "is_auto_thinking_level"
    )]
    pub thinking_level: String,
    #[serde(
        default = "default_thinking_format",
        skip_serializing_if = "is_auto_thinking_format"
    )]
    pub thinking_format: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub extra_body: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptConfig {
    #[serde(default = "default_prompts_dir")]
    pub prompts_dir: String,
    #[serde(default = "default_identities_dir")]
    pub identities_dir: String,
    #[serde(default = "default_user_identity_file")]
    pub user_identity_file: String,
    #[serde(default)]
    pub active_persona: String,
    #[serde(default)]
    pub active_identity: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GatewayConfig {
    #[serde(default)]
    pub qq: QqGatewayConfig,
    #[serde(default)]
    pub weixin: WeixinGatewayConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QqGatewayConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_qq_gateway_transport")]
    pub transport: String,
    #[serde(default = "default_qq_gateway_listen")]
    pub listen: String,
    #[serde(default = "default_qq_gateway_base_url")]
    pub base_url: String,
    #[serde(default)]
    pub token: String,
    #[serde(default)]
    pub app_id: String,
    #[serde(default)]
    pub client_secret: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WeixinGatewayConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_weixin_gateway_base_url")]
    pub base_url: String,
    #[serde(default = "default_weixin_gateway_cdn_base_url")]
    pub cdn_base_url: String,
    #[serde(default = "default_weixin_gateway_bot_type")]
    pub bot_type: String,
    #[serde(default)]
    pub token: String,
    #[serde(default)]
    pub account: String,
    #[serde(default)]
    pub bot_agent: String,
}

#[derive(Debug, Clone)]
pub struct ProviderModelChoice {
    pub provider_id: String,
    pub provider_name: String,
    pub model: String,
}

impl ProviderModelChoice {
    pub fn value(&self) -> String {
        format!("{}\t{}", self.provider_id, self.model)
    }

    pub fn label(&self) -> String {
        format!("{} / {}", self.provider_name, self.model)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextConfig {
    #[serde(default = "default_context_chars")]
    pub default_max_chars: usize,
    /// 压缩专用供应商；留空时沿用当前会话供应商。
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub compaction_provider_id: String,
    /// 压缩专用模型；留空时沿用当前会话模型。
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub compaction_model: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolsConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub max_rounds: usize,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub command_shell: String,
    #[serde(default)]
    pub progressive_loading_enabled: bool,
    #[serde(default = "default_true")]
    pub background_commands_enabled: bool,
    #[serde(default = "default_background_command_timeout_seconds")]
    pub background_command_timeout_seconds: u64,
    #[serde(default = "default_background_command_log_max_bytes")]
    pub background_command_log_max_bytes: u64,
    #[serde(default = "default_background_command_stop_grace_seconds")]
    pub background_command_stop_grace_seconds: u64,
}

/// 网页终端配置。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TerminalConfig {
    /// 终端 Shell 可执行文件路径或名称，留空时使用平台默认值。
    #[serde(default)]
    pub shell: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillsConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub allow_command_execution: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MemoryConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_true")]
    pub evicted_context_enabled: bool,
    #[serde(default = "default_true")]
    pub association_enabled: bool,
    #[serde(default = "default_true")]
    pub auto_diary_enabled: bool,
    #[serde(default = "default_true")]
    pub auto_fact_enabled: bool,
    #[serde(default = "default_true")]
    pub auto_skill_enabled: bool,
    #[serde(default = "default_memory_association_facts")]
    pub association_facts: usize,
    #[serde(default = "default_memory_association_episodes")]
    pub association_episodes: usize,
    #[serde(default = "default_memory_association_max_chars")]
    pub association_max_chars: usize,
    #[serde(default = "default_memory_snippet_chars")]
    pub snippet_chars: usize,
    #[serde(default = "default_memory_forget_after_days")]
    pub forget_after_days: u64,
    #[serde(default = "default_true")]
    pub forgetting_enabled: bool,
    #[serde(default = "default_memory_half_life_days")]
    pub forgetting_half_life_days: f64,
    #[serde(default = "default_memory_min_strength")]
    pub forgetting_min_strength: f64,
    #[serde(default = "default_memory_review_boost")]
    pub forgetting_review_boost: f64,
    #[serde(default = "default_memory_min_task_chars")]
    pub learning_min_task_chars: usize,
    #[serde(default = "default_memory_min_method_chars")]
    pub learning_min_method_chars: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginsConfig {
    #[serde(default)]
    pub weather: PluginEnabledConfig,
    #[serde(default)]
    pub web: WebPluginConfig,
    #[serde(default)]
    pub web_images: WebImagesPluginConfig,
    #[serde(default)]
    pub deep_research: DeepResearchPluginConfig,
    #[serde(default)]
    pub deep_diagnose: DeepDiagnosePluginConfig,
    #[serde(default)]
    pub vision: VisionPluginConfig,
    #[serde(default)]
    pub exchange_rate: ExchangeRatePluginConfig,
    #[serde(default)]
    pub xuanxue: PluginEnabledConfig,
    #[serde(default)]
    pub image_generation: ImageGenerationPluginConfig,
    #[serde(default)]
    pub print_image: PrintImagePluginConfig,
    #[serde(default)]
    pub memes: MemesPluginConfig,
    #[serde(default)]
    pub knowledge_base: KnowledgeBasePluginConfig,
    #[serde(default)]
    pub archlinux: PluginEnabledConfig,
    #[serde(default)]
    pub man: PluginEnabledConfig,
    #[serde(default)]
    pub moegirl: PluginEnabledConfig,
    #[serde(default)]
    pub hash_codec: PluginEnabledConfig,
    #[serde(default)]
    pub calculator: CalculatorPluginConfig,
    #[serde(default)]
    pub package_advisor: PluginEnabledConfig,
    #[serde(default)]
    pub linux_game_compatibility: LinuxGameCompatibilityConfig,
    #[serde(default)]
    pub diagnostics: DiagnosticsPluginConfig,
    #[serde(default)]
    pub memory: MemoryConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginEnabledConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LinuxGameCompatibilityConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_subagent_max_tool_steps")]
    pub max_tool_steps: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebPluginConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub tinyfish_api_keys: Vec<String>,
    #[serde(default)]
    pub tavily_api_keys: Vec<String>,
    #[serde(default)]
    pub firecrawl_api_keys: Vec<String>,
    #[serde(default)]
    pub anysearch_api_keys: Vec<String>,
    #[serde(default)]
    pub searxng_base_url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebImagesPluginConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_web_images_max_results")]
    pub max_results: usize,
    #[serde(default = "default_web_images_max_download_mb")]
    pub max_download_mb: f64,
    #[serde(default = "default_true")]
    pub safe_search: bool,
    #[serde(default = "default_true")]
    pub vision_screening_enabled: bool,
    #[serde(default = "default_true")]
    pub auto_preview: bool,
    #[serde(default = "default_web_images_preview_count")]
    pub preview_count: usize,
    #[serde(default = "default_web_images_timeout")]
    pub timeout_seconds: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeepResearchPluginConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_deep_research_dir")]
    pub output_dir: String,
    #[serde(default = "default_deep_research_depth")]
    pub thinking_depth: String,
    #[serde(default = "default_deep_research_max_review_revisions")]
    pub max_review_revisions: usize,
    #[serde(default = "default_deep_research_max_tool_steps")]
    pub max_tool_steps_per_round: usize,
    #[serde(default)]
    pub max_final_answer_chars: usize,
    #[serde(default = "default_deep_research_tool_timeout")]
    pub tool_call_timeout_seconds: u64,
    #[serde(default = "default_true")]
    pub show_progress: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeepDiagnosePluginConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_deep_research_depth")]
    pub thinking_depth: String,
    #[serde(default = "default_deep_research_max_review_revisions")]
    pub max_review_revisions: usize,
    #[serde(default = "default_deep_research_max_tool_steps")]
    pub max_tool_steps_per_round: usize,
    #[serde(default)]
    pub max_final_answer_chars: usize,
    #[serde(default = "default_deep_research_tool_timeout")]
    pub tool_call_timeout_seconds: u64,
    #[serde(default = "default_subagent_max_tool_steps")]
    pub max_tool_steps: usize,
    #[serde(default = "default_true")]
    pub show_progress: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VisionPluginConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_true")]
    pub prefer_current_multimodal_model: bool,
    #[serde(default)]
    pub vision_provider_id: String,
    #[serde(default)]
    pub vision_model: String,
    #[serde(default = "default_true")]
    pub preview_with_chafa: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExchangeRatePluginConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub api_key: String,
    #[serde(default = "default_true")]
    pub free_fallback_enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageGenerationPluginConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_image_generation_provider_type")]
    pub provider_type: String,
    #[serde(default = "default_openai_images_base_url")]
    pub base_url: String,
    #[serde(default)]
    pub api_keys: Vec<String>,
    #[serde(default = "default_image_generation_model")]
    pub model: String,
    #[serde(default = "default_image_generation_aspect_ratio")]
    pub default_aspect_ratio: String,
    #[serde(default = "default_image_generation_resolution")]
    pub default_resolution: String,
    #[serde(default = "default_image_generation_output_dir")]
    pub output_dir: String,
    #[serde(default)]
    pub auto_print: bool,
    #[serde(default = "default_image_generation_timeout")]
    pub timeout_seconds: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrintImagePluginConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_print_image_width_percent")]
    pub width_percent: u8,
    #[serde(default = "default_print_image_height_percent")]
    pub height_percent: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemesPluginConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub persona_libraries: HashMap<String, String>,
    #[serde(default = "default_memes_width_percent")]
    pub width_percent: u8,
    #[serde(default = "default_memes_height_percent")]
    pub height_percent: u8,
    #[serde(default = "default_memes_max_image_mb")]
    pub max_image_mb: u64,
    #[serde(default)]
    pub allow_gif_animation: bool,
    #[serde(default)]
    pub auto_send_enabled: bool,
    #[serde(default = "default_memes_auto_send_probability")]
    pub auto_send_probability: f32,
    #[serde(default = "default_memes_auto_send_min_confidence")]
    pub auto_send_min_confidence: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KnowledgeBasePluginConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub data_dir: String,
    #[serde(default = "default_kb_max_search_results")]
    pub max_search_results: usize,
    #[serde(default = "default_kb_snippet_context_chars")]
    pub snippet_context_chars: usize,
    #[serde(default = "default_kb_proximity_window_chars")]
    pub proximity_window_chars: usize,
    #[serde(default = "default_kb_max_read_lines")]
    pub max_read_lines: usize,
    #[serde(default = "default_kb_max_file_size_kb")]
    pub max_file_size_kb: usize,
    #[serde(default = "default_kb_allowed_extensions")]
    pub allowed_extensions: String,
    #[serde(default = "default_kb_allowed_filenames")]
    pub allowed_filenames: String,
    #[serde(default = "default_true")]
    pub upload_tool_enabled: bool,
    #[serde(default = "default_true")]
    pub embedding_enabled: bool,
    #[serde(default)]
    pub embedding_provider_id: String,
    #[serde(default)]
    pub embedding_model: String,
    #[serde(default = "default_kb_semantic_chunk_chars")]
    pub semantic_chunk_chars: usize,
    #[serde(default = "default_kb_semantic_chunk_overlap")]
    pub semantic_chunk_overlap: usize,
    #[serde(default = "default_kb_semantic_top_k")]
    pub semantic_top_k: usize,
    #[serde(default = "default_kb_semantic_min_score")]
    pub semantic_min_score: f32,
    #[serde(default = "default_kb_keyword_strong_score_threshold")]
    pub keyword_strong_score_threshold: f32,
    #[serde(default = "default_kb_embedding_timeout_seconds")]
    pub embedding_timeout_seconds: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CalculatorPluginConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_calculator_backend")]
    pub backend: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiagnosticsPluginConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_diagnostics_timeout")]
    pub command_timeout_seconds: u64,
    #[serde(default = "default_diagnostics_max_stdout_chars")]
    pub max_stdout_chars: usize,
    #[serde(default = "default_diagnostics_max_stderr_chars")]
    pub max_stderr_chars: usize,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SecretsConfig {
    #[serde(default)]
    pub api_keys: HashMap<String, String>,
}

/// 对话生命周期 Hook 配置（参考 LiveAgent）。
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct HooksConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub items: Vec<HookItem>,
}

/// 单条 Hook 定义。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HookItem {
    pub name: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// agent_start / agent_end / turn_start / turn_end / tool_execution_start / tool_execution_end
    pub event: String,
    /// command | http
    #[serde(default = "default_hook_kind")]
    pub kind: String,
    #[serde(default)]
    pub script: String,
    #[serde(default)]
    pub timeout_ms: Option<u64>,
    #[serde(default)]
    pub requests: Vec<HookHttpRequest>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HookHttpRequest {
    #[serde(default)]
    pub id: String,
    pub url: String,
    #[serde(default = "default_http_method")]
    pub method: String,
    #[serde(default)]
    pub headers: HashMap<String, String>,
    #[serde(default)]
    pub body: String,
}

/// MCP 服务器配置。
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct McpConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub servers: Vec<McpServerConfig>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct McpServerConfig {
    pub id: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// stdio | http | sse
    #[serde(default = "default_mcp_transport")]
    pub transport: String,
    #[serde(default)]
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub env: HashMap<String, String>,
    #[serde(default)]
    pub cwd: Option<String>,
    /// HTTP/SSE 端点 URL
    #[serde(default)]
    pub url: Option<String>,
    /// SSE 可选的 message URL；缺省时从 SSE 握手事件解析
    #[serde(default)]
    pub message_url: Option<String>,
    #[serde(default)]
    pub headers: HashMap<String, String>,
    #[serde(default)]
    pub timeout_ms: Option<u64>,
}

fn default_hook_kind() -> String {
    "command".to_string()
}

fn default_http_method() -> String {
    "POST".to_string()
}

fn default_mcp_transport() -> String {
    "stdio".to_string()
}
