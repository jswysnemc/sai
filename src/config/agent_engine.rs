use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

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
    /// Claude Code（@zed-industries/claude-code-acp）
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

    /// 返回该内核下不可用的 sai 功能。
    ///
    /// 外部内核自己维护对话历史与上下文，sai 在轮次里注入的那部分能力因此失效。
    /// 这份清单是三端共用的事实来源：切换内核时要让用户知道换掉了什么，
    /// 否则压缩与记忆静默停摆，用起来像是出了 bug。
    ///
    /// 返回:
    /// - 失效功能的说明；原生内核为空
    pub fn unavailable_features(self) -> &'static [&'static str] {
        if !self.is_external() {
            return &[];
        }
        // 与 i18n::text 一样按当前语言给出，直接用于界面展示
        if crate::i18n::is_zh() {
            &[
                "上下文压缩",
                "记忆注入",
                "目标续轮",
                "子智能体",
                "token 用量统计",
            ]
        } else {
            &[
                "context compaction",
                "memory injection",
                "goal continuation",
                "subagents",
                "token usage stats",
            ]
        }
    }

    /// 返回预置内核的默认启动命令。
    ///
    /// 版本不写死：适配器仍在快速迭代，交给 npx 解析最新版本，
    /// 需要固定版本时用 `custom` 自行指定。
    ///
    /// 返回:
    /// - `(程序, 参数)`；原生与自定义内核没有预置命令
    pub fn preset_command(self) -> Option<(&'static str, &'static [&'static str])> {
        match self {
            Self::ClaudeCode => Some(("npx", &["-y", "@zed-industries/claude-code-acp"])),
            Self::Codex => Some(("npx", &["-y", "@agentclientprotocol/codex-acp"])),
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
}

impl Default for AcpEngineConfig {
    fn default() -> Self {
        Self {
            command: String::new(),
            args: Vec::new(),
            env: BTreeMap::new(),
            startup_timeout_seconds: default_startup_timeout_seconds(),
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
        let (program, args) = self.engine.preset_command()?;
        Some((
            program.to_string(),
            args.iter().map(|arg| arg.to_string()).collect(),
        ))
    }
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
        assert!(args.iter().any(|arg| arg.contains("claude-code-acp")));

        let codex: AgentEngineConfig = serde_json::from_str(r#"{"engine":"codex"}"#).unwrap();
        let (_, args) = codex.resolved_command().unwrap();
        assert!(args.iter().any(|arg| arg.contains("codex-acp")));
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
