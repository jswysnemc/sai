use super::agents::{AgentProfile, AgentRuntimeOverride, SubagentConfig};
use super::cli_tools::PluginsConfig;
use super::defaults::*;
use super::git::{GitConfig, ScmConfig};
use super::model::*;
use super::permission::PermissionConfig;
use super::session::SessionConfig;
use serde::{Deserialize, Serialize};

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
    pub context: ContextConfig,
    /// 模型请求瞬时失败的自动重试策略
    #[serde(default)]
    pub retry: RetryConfig,
    #[serde(default)]
    pub tools: ToolsConfig,
    #[serde(default)]
    pub terminal: TerminalConfig,
    #[serde(default)]
    pub input: InputConfig,
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

/// 【应用配置】【指令默认值】缺省加载工作区与用户指令文件
/// @returns 新配置的指令加载开关
fn default_load_instruction_files() -> bool {
    true
}
