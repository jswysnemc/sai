use super::agents::{AgentProfile, AgentRuntimeOverride, SubagentConfig};
use super::cli_tools::PluginsConfig;
use super::defaults::*;
use super::git::{GitConfig, ScmConfig};
use super::model_metadata::ModelMetadata;
use super::notification::NotificationConfig;
use super::permission::PermissionConfig;
use super::prompt_templates::PromptTemplatesConfig;
use super::session::SessionConfig;
use serde::{Deserialize, Deserializer, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub active_provider: String,
    pub providers: Vec<ProviderConfig>,
    /// 执行对话轮次的内核：sai 自带或外部 ACP agent
    #[serde(default)]
    pub agent: crate::config::AgentEngineConfig,
    #[serde(default)]
    pub permission: PermissionConfig,
    #[serde(default)]
    pub session: SessionConfig,
    #[serde(default)]
    pub notification: NotificationConfig,
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
    /// HTTP 请求与响应调试记录配置。
    #[serde(default)]
    pub debug: DebugConfig,
    #[serde(default)]
    pub scm: ScmConfig,
    #[serde(default)]
    pub git: GitConfig,
    /// SSH 主机列表，供 Web 终端建立远程会话
    #[serde(default)]
    pub ssh: crate::config::SshConfig,
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
    /// MCP 配置在运行时从独立 `mcp.jsonc` 注入；主配置文件不再写出该字段。
    /// 读取 `config.jsonc` 时仍可解析 legacy `mcp` 段用于迁移。
    #[serde(default, skip_serializing)]
    pub mcp: McpConfig,
    #[serde(default)]
    pub plugins: PluginsConfig,
    #[serde(default, skip_serializing)]
    pub memory: MemoryConfig,
    #[serde(default)]
    pub system_prompt_file: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub system_prompt: Option<String>,
    /// 是否加载全局 / 项目 AGENT.md 等指令文件（可由 Agent 档案覆盖）
    #[serde(default = "default_load_instruction_files", skip_serializing)]
    pub load_instruction_files: bool,
    /// 运行期生效的提示词分段开关，由 Agent 档案写入
    #[serde(default)]
    pub prompt_sections: super::prompt_sections::PromptSectionToggles,
    /// 会话网格：跨会话消息收发开关，默认只允许投递给自己
    #[serde(default)]
    pub mesh: super::mesh::MeshConfig,
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

/// Web 可控制的调试记录配置。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DebugConfig {
    /// 是否启用 HTTP 请求调试记录。
    #[serde(default)]
    pub enabled: bool,
    /// 是否保留完整请求、响应流和重组响应。
    #[serde(default = "default_true")]
    pub retain_logs: bool,
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
    /// 是否启用；停用后不参与模型选择，也不能被解析为当前供应商
    #[serde(default = "default_true", skip_serializing_if = "is_default_true")]
    pub enabled: bool,
    #[serde(
        default = "default_provider_protocol",
        skip_serializing_if = "is_auto_protocol"
    )]
    pub protocol: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_key: Option<String>,
    /// 多密钥列表；非空时优先于 `api_key` 单值
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub api_keys: Vec<super::provider_keys::ProviderApiKey>,
    /// 手动选中的密钥标识；负载均衡关闭时生效
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_key_selected: Option<String>,
    /// 手动开启负载均衡：在 `api_keys` 之间轮询
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub api_key_balance: bool,
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f64>,
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
    /// 是否在多轮对话中回传历史思考内容（Moonshot 的 Preserved Thinking）。
    ///
    /// kimi-k2.7-code 等模型强制开启该行为，多轮请求必须原样带回历史
    /// `reasoning_content`，否则服务端报错；kimi-k2.6 通过 `thinking.keep`
    /// 显式开启。默认关闭，避免给不支持的供应商发出多余字段。
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub preserve_thinking: bool,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub extra_body: String,
    /// 附加到每次模型请求的自定义 HTTP 头（不含 Authorization）。
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub extra_headers: HashMap<String, String>,
    /// 自定义 User-Agent；空时：`codex` 用 Codex CLI UA，`claude` 用 Claude CLI UA，其它用默认客户端 UA。
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub user_agent: String,
    /// 客户端模拟：`auto` | `default` | `codex` | `claude`（Claude Code 请求头与 Messages 形态）。
    #[serde(
        default = "default_client_style",
        skip_serializing_if = "is_auto_client_style"
    )]
    pub client_style: String,
    /// Claude Code 模拟时是否启用 1M 上下文 beta（`context-1m-2025-08-07`）。
    #[serde(
        default = "default_claude_1m_context",
        skip_serializing_if = "is_default_claude_1m_context"
    )]
    pub claude_1m_context: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptConfig {
    #[serde(default = "default_identities_dir")]
    pub identities_dir: String,
    #[serde(default = "default_user_identity_file")]
    pub user_identity_file: String,
    #[serde(default)]
    pub active_identity: String,
    /// 提交说明、会话标题和上下文压缩使用的内部提示词
    #[serde(default)]
    pub templates: PromptTemplatesConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GatewayConfig {
    #[serde(default)]
    pub qq: QqGatewayConfig,
    #[serde(default)]
    pub weixin: WeixinGatewayConfig,
    #[serde(default)]
    pub feishu: FeishuGatewayConfig,
}

/// 飞书（Lark）机器人网关配置。
///
/// 走开放平台的事件订阅：飞书把消息 POST 到这里，回复经 open-apis 发出。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeishuGatewayConfig {
    #[serde(default)]
    pub enabled: bool,
    /// 事件订阅监听地址
    #[serde(default = "default_feishu_gateway_listen")]
    pub listen: String,
    /// 开放平台接口地址；私有化部署时改这里
    #[serde(default = "default_feishu_gateway_base_url")]
    pub base_url: String,
    /// 应用凭据
    #[serde(default)]
    pub app_id: String,
    #[serde(default)]
    pub app_secret: String,
    /// 事件订阅的验证 token；配置了才校验来源
    #[serde(default)]
    pub verification_token: String,
    /// 事件加密密钥；开放平台开启加密时必填
    #[serde(default)]
    pub encrypt_key: String,
}

impl Default for FeishuGatewayConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            listen: default_feishu_gateway_listen(),
            base_url: default_feishu_gateway_base_url(),
            app_id: String::new(),
            app_secret: String::new(),
            verification_token: String::new(),
            encrypt_key: String::new(),
        }
    }
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

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct ProviderModelChoice {
    pub provider_id: String,
    pub provider_name: String,
    pub model: String,
}

impl ProviderModelChoice {
    /// 返回可用于稳定比较的供应商与模型组合值。
    ///
    /// 返回:
    /// - 供应商标识与模型名称组合
    pub fn value(&self) -> String {
        format!("{}\t{}", self.provider_id, self.model)
    }

    /// 返回包含供应商展示名的模型标签。
    ///
    /// 返回:
    /// - `供应商 / 模型` 展示文本
    pub fn label(&self) -> String {
        let provider = if self.provider_name.trim().is_empty() {
            &self.provider_id
        } else {
            &self.provider_name
        };
        format!("{provider} / {}", self.model)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextConfig {
    #[serde(default = "default_context_chars")]
    pub default_max_chars: usize,
    /// 自动压缩比例，占用达到窗口的该比例时触发。
    #[serde(
        default = "default_compaction_ratio",
        skip_serializing_if = "is_default_compaction_ratio"
    )]
    pub compaction_ratio: f32,
    /// 自动压缩预留 token；0 表示只按比例。大窗口按剩余量触发。
    #[serde(
        default = "default_compaction_reserve_tokens",
        skip_serializing_if = "is_default_compaction_reserve_tokens"
    )]
    pub compaction_reserve_tokens: usize,
    /// 压缩专用供应商；留空时沿用当前会话供应商。
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub compaction_provider_id: String,
    /// 压缩专用模型；留空时沿用当前会话模型。
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub compaction_model: String,
}

impl ContextConfig {
    /// 夹紧后的自动压缩比例。
    ///
    /// 返回:
    /// - 0.50–0.99 之间的比例
    pub fn clamped_compaction_ratio(&self) -> f32 {
        parse_compaction_ratio_value(self.compaction_ratio)
    }
}

/// 自动压缩比例下限。
pub const MIN_COMPACTION_RATIO: f32 = 0.50;
/// 自动压缩比例上限。
pub const MAX_COMPACTION_RATIO: f32 = 0.99;

/// 把数值比例夹紧到合法区间；非有限值回退默认 0.9。
///
/// 参数:
/// - `ratio`: 原始比例
///
/// 返回:
/// - 0.50–0.99 之间的比例
pub fn parse_compaction_ratio_value(ratio: f32) -> f32 {
    if !ratio.is_finite() {
        return default_compaction_ratio();
    }
    ratio.clamp(MIN_COMPACTION_RATIO, MAX_COMPACTION_RATIO)
}

/// 解析表单里的压缩比例，支持 `0.9`、`90`、`90%`。
///
/// 参数:
/// - `value`: 用户输入
///
/// 返回:
/// - 夹紧后的比例；无法解析时返回错误
pub fn parse_compaction_ratio_text(value: &str) -> anyhow::Result<f32> {
    let trimmed = value.trim().trim_end_matches('%').trim();
    let parsed = trimmed.parse::<f32>().map_err(|_| {
        anyhow::anyhow!("context.compaction_ratio is invalid: {value}")
    })?;
    let ratio = if parsed > 1.0 { parsed / 100.0 } else { parsed };
    if !ratio.is_finite() || ratio < MIN_COMPACTION_RATIO || ratio > MAX_COMPACTION_RATIO {
        anyhow::bail!(
            "context.compaction_ratio must be between {MIN_COMPACTION_RATIO} and {MAX_COMPACTION_RATIO}"
        );
    }
    Ok(ratio)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolsConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// 历史字段：旧配置可能仍带此键，运行时不再限制工具轮次。
    #[serde(default, skip_serializing_if = "is_zero_max_rounds")]
    pub max_rounds: usize,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub command_shell: String,
    /// 命令输出过滤器：auto（探测到 rtk 时启用）/ rtk（强制）/ off（关闭）。
    #[serde(default = "default_command_filter")]
    pub command_filter: String,
    /// 不交给 rtk 接管的命令；留空表示 rtk 能代理的命令全部交给它。
    ///
    /// 代理范围由 rtk 自身决定（运行时探测其子命令并逐条询问映射），
    /// 本列表只用于把个别命令排除在外。
    ///
    /// 语义与旧的 `command_filter_allowlist` 相反，因此不做字段别名兼容：
    /// 沿用旧字段值会把「只让这些走 rtk」误读成「这些不走 rtk」。
    #[serde(default)]
    pub command_filter_denylist: Vec<String>,
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
    /// 是否把记忆索引注入每轮上下文
    #[serde(default = "default_true")]
    pub association_enabled: bool,
    /// 逐出上下文检索时片段的最大字符数
    #[serde(default = "default_memory_snippet_chars")]
    pub snippet_chars: usize,
    /// 会话记忆点提取专用供应商；留空时沿用当前会话供应商。
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub extraction_provider_id: String,
    /// 会话记忆点提取专用模型；留空时沿用当前会话模型。
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub extraction_model: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SecretsConfig {
    #[serde(default)]
    pub api_keys: HashMap<String, String>,
    /// Web 访问口令的 Argon2 哈希；为空表示未启用口令验证
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub web_password_hash: Option<String>,
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

fn default_load_instruction_files() -> bool {
    true
}
