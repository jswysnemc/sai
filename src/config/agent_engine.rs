use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use std::path::PathBuf;

const CODEX_SIDECAR_INDEX_SOURCE: &str =
    include_str!("../../sidecars/codex-agent-acp/src/index.js");
const CODEX_SIDECAR_CAPABILITY_SOURCE: &str =
    include_str!("../../sidecars/codex-agent-acp/src/capability-extensions.js");

/// 执行对话轮次的内核。
///
/// `Native` 是 sai 自带的 LLM 循环；其余取值把轮次交给外部 ACP agent 执行，
/// sai 退居客户端，仍然负责权限、沙箱、审计与会话持久化。
#[derive(Debug, Clone, Copy, Eq, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum AgentEngineKind {
    /// sai 自带内核
    #[default]
    Native,
    /// Claude Code（Sai Sidecar / @agentclientprotocol/claude-agent-acp）
    ClaudeCode,
    /// Codex（@agentclientprotocol/codex-acp）
    Codex,
    /// 自定义 ACP agent，启动命令由 `acp` 段给出
    Custom,
}

impl AgentEngineKind {
    /// 返回配置文件与协议使用的稳定名称。
    ///
    /// 返回:
    /// - 小写内核名
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Native => "native",
            Self::ClaudeCode => "claude_code",
            Self::Codex => "codex",
            Self::Custom => "custom",
        }
    }

    /// 判断是否需要启动外部 ACP 进程。
    ///
    /// 返回:
    /// - 非原生内核返回 true
    pub fn is_external(self) -> bool {
        !matches!(self, Self::Native)
    }

    /// 返回内核的展示名称。
    ///
    /// 返回:
    /// - 界面上展示给用户的名称
    pub fn display_label(self) -> &'static str {
        match self {
            Self::Native => crate::i18n::text("Native", "内置内核"),
            Self::ClaudeCode => "Claude Code",
            Self::Codex => "Codex",
            Self::Custom => crate::i18n::text("Custom ACP agent", "自定义 ACP 内核"),
        }
    }

    /// 返回当前握手能力下不可用的 Sai 功能。
    ///
    /// 外部内核可以通过 initialize 的 `_sai.capabilities` 声明已经适配的
    /// Sai 专属能力；未握手或未声明时保持保守禁用。
    ///
    /// 返回:
    /// - 当前不可用功能的本地化名称
    pub fn unavailable_features(self) -> Vec<&'static str> {
        if !self.is_external() {
            return Vec::new();
        }
        let capabilities = crate::acp::current_capabilities(self.as_str()).unwrap_or_default();
        let candidates = if crate::i18n::is_zh() {
            [
                ("上下文压缩", capabilities.sai_context_compaction),
                ("记忆注入", capabilities.sai_memory),
                ("活动目标延续", capabilities.sai_goal_continuation),
                ("子智能体", capabilities.sai_subagents),
            ]
        } else {
            [
                ("context compaction", capabilities.sai_context_compaction),
                ("memory injection", capabilities.sai_memory),
                ("goal continuation", capabilities.sai_goal_continuation),
                ("subagents", capabilities.sai_subagents),
            ]
        };
        candidates
            .into_iter()
            .filter_map(|(name, available)| (!available).then_some(name))
            .collect()
    }

    /// 返回预置内核的默认启动命令。
    ///
    /// 预置命令固定经过验证的适配器版本。
    ///
    /// 返回:
    /// - `(程序, 参数)`；原生与自定义内核没有预置命令
    pub fn preset_command(self) -> Option<(&'static str, &'static [&'static str])> {
        match self {
            Self::ClaudeCode => Some(("npx", &["-y", "@agentclientprotocol/claude-agent-acp"])),
            Self::Codex => Some(("npx", &["-y", "@agentclientprotocol/codex-acp@1.1.7"])),
            Self::Native | Self::Custom => None,
        }
    }
}

/// 外部 ACP agent 的启动配置。
#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct AcpEngineConfig {
    /// 启动程序；留空时使用所选内核的预置命令
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub command: String,
    /// 启动参数；`command` 留空时同样使用预置值
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub args: Vec<String>,
    /// 追加到子进程的环境变量
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub env: BTreeMap<String, String>,
    /// 握手超时秒数；首次运行要下载适配器，默认给得较宽
    #[serde(default = "default_startup_timeout_seconds")]
    pub startup_timeout_seconds: u64,
    /// 传给 ACP 会话的附加工作区目录
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub additional_directories: Vec<PathBuf>,
    /// initialize 后主动调用的 ACP 认证方式标识
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub auth_method: String,
    /// 按 `model` 类别匹配并设置的模型值
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub model: String,
    /// 按 `mode` 类别匹配并设置的权限模式值
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub permission_mode: String,
    /// 按 `thought_level` 类别匹配并设置的思考等级值
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub thought_level: String,
    /// 以配置项 id 为键的任意 ACP session configOptions 覆盖
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub config_options: BTreeMap<String, Value>,
}

impl Default for AcpEngineConfig {
    fn default() -> Self {
        Self {
            command: String::new(),
            args: Vec::new(),
            env: BTreeMap::new(),
            startup_timeout_seconds: default_startup_timeout_seconds(),
            additional_directories: Vec::new(),
            auth_method: String::new(),
            model: String::new(),
            permission_mode: String::new(),
            thought_level: String::new(),
            config_options: BTreeMap::new(),
        }
    }
}

/// 默认握手超时秒数。
fn default_startup_timeout_seconds() -> u64 {
    90
}

/// 对话内核配置。
#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize, Default)]
pub struct AgentEngineConfig {
    /// 使用哪个内核执行对话轮次
    #[serde(default)]
    pub engine: AgentEngineKind,
    /// 外部内核的启动配置
    #[serde(default)]
    pub acp: AcpEngineConfig,
}

impl AgentEngineConfig {
    /// 解析实际生效的启动命令。
    ///
    /// 显式配置优先于预置命令，便于固定适配器版本或换用本地构建。
    ///
    /// 返回:
    /// - `(程序, 参数)`；原生内核或缺少命令时返回 None
    pub fn resolved_command(&self) -> Option<(String, Vec<String>)> {
        if !self.engine.is_external() {
            return None;
        }
        let command = self.acp.command.trim();
        if !command.is_empty() {
            return Some((command.to_string(), self.acp.args.clone()));
        }
        if self.engine == AgentEngineKind::ClaudeCode {
            let sidecar = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("sidecars")
                .join("claude-agent-acp");
            if sidecar.join("package.json").is_file() {
                return Some((
                    "npx".to_string(),
                    vec![
                        "-y".to_string(),
                        "--package".to_string(),
                        sidecar.display().to_string(),
                        "sai-claude-agent-acp".to_string(),
                    ],
                ));
            }
        }
        if self.engine == AgentEngineKind::Codex {
            let sidecar = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("sidecars")
                .join("codex-agent-acp");
            let entry = sidecar.join("src").join("index.js");
            if entry.is_file() {
                return Some(("node".to_string(), vec![entry.display().to_string()]));
            }
            return Some(embedded_codex_sidecar_command());
        }
        let (program, args) = self.engine.preset_command()?;
        Some((
            program.to_string(),
            args.iter().map(|arg| arg.to_string()).collect(),
        ))
    }
}

/// 构造不依赖源码目录的内嵌 Codex Sidecar 启动命令。
///
/// 发布产物只包含 Sai 可执行文件，因此通过 Node data URL 载入编译时嵌入的
/// Sidecar 模块；模块内部仍会下载固定版本的 Codex ACP 适配器。
///
/// 返回:
/// - Node 程序与内嵌 Sidecar 参数
fn embedded_codex_sidecar_command() -> (String, Vec<String>) {
    // 【Codex ACP Sidecar】【发布回退】1. 将两个内嵌模块编码成可安全拼入脚本的字符串
    let extension_source = Value::String(CODEX_SIDECAR_CAPABILITY_SOURCE.to_string()).to_string();
    let index_source = Value::String(CODEX_SIDECAR_INDEX_SOURCE.to_string()).to_string();
    // 【Codex ACP Sidecar】【发布回退】2. 通过 data URL 恢复模块依赖关系并启动 Sidecar
    let script = format!(
        "const extensionSource={extension_source};\n\
         const extensionUrl='data:text/javascript;base64,'+Buffer.from(extensionSource).toString('base64');\n\
         const indexSource={index_source}.replace('./capability-extensions.js',extensionUrl);\n\
         const indexUrl='data:text/javascript;base64,'+Buffer.from(indexSource).toString('base64');\n\
         const sidecar=await import(indexUrl);\n\
         sidecar.main();"
    );
    // 【Codex ACP Sidecar】【发布回退】3. 使用 Node.js 模块模式执行内嵌启动脚本
    (
        "node".to_string(),
        vec![
            "--input-type=module".to_string(),
            "--eval".to_string(),
            script,
        ],
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_to_native_engine() {
        let config: AgentEngineConfig = serde_json::from_str("{}").unwrap();
        assert_eq!(config.engine, AgentEngineKind::Native);
        assert!(!config.engine.is_external());
        assert!(config.resolved_command().is_none());
    }

    #[test]
    fn preset_engines_resolve_their_launch_command() {
        let config: AgentEngineConfig =
            serde_json::from_str(r#"{"engine":"claude_code"}"#).unwrap();
        let (program, args) = config.resolved_command().unwrap();
        assert_eq!(program, "npx");
        assert!(args.iter().any(|arg| arg.contains("claude-agent-acp")));

        let codex: AgentEngineConfig = serde_json::from_str(r#"{"engine":"codex"}"#).unwrap();
        let (program, args) = codex.resolved_command().unwrap();
        assert_eq!(program, "node");
        assert!(args
            .iter()
            .any(|arg| arg.contains("codex-agent-acp/src/index.js")));
    }

    /// 发布版内嵌命令必须同时保留 Sidecar 扩展和固定适配器版本。
    #[test]
    fn embedded_codex_sidecar_keeps_capabilities_and_pinned_version() {
        let (program, args) = embedded_codex_sidecar_command();
        let script = args.last().unwrap();

        assert_eq!(program, "node");
        assert!(script.contains("native_equivalents"));
        assert!(script.contains("@agentclientprotocol/codex-acp@1.1.7"));
    }

    #[test]
    fn explicit_command_overrides_preset() {
        let config: AgentEngineConfig = serde_json::from_str(
            r#"{"engine":"claude_code","acp":{"command":"/opt/acp","args":["--stdio"]}}"#,
        )
        .unwrap();
        let (program, args) = config.resolved_command().unwrap();
        assert_eq!(program, "/opt/acp");
        assert_eq!(args, vec!["--stdio".to_string()]);
    }

    #[test]
    fn custom_engine_without_command_has_nothing_to_launch() {
        let config: AgentEngineConfig = serde_json::from_str(r#"{"engine":"custom"}"#).unwrap();
        assert!(config.engine.is_external());
        assert!(config.resolved_command().is_none());
    }
}
