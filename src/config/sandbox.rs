use serde::{Deserialize, Serialize};

/// 沙箱内命令的网络策略。
#[derive(Debug, Clone, Copy, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SandboxNetworkMode {
    /// 断开网络命名空间；需要联网的命令走提升审批
    #[default]
    Deny,
    /// 保留主机网络，只限制文件系统
    Allow,
}

impl SandboxNetworkMode {
    /// 返回配置文件使用的稳定字符串。
    ///
    /// 返回:
    /// - `deny` 或 `allow`
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Deny => "deny",
            Self::Allow => "allow",
        }
    }

    /// 从配置界面字符串解析网络策略。
    ///
    /// 参数:
    /// - `value`: 配置字符串
    ///
    /// 返回:
    /// - 已识别策略；未知值回退到断网
    pub fn parse_or_default(value: &str) -> Self {
        match value.trim() {
            "allow" => Self::Allow,
            _ => Self::Deny,
        }
    }
}

/// 审核模式与计划模式下 Shell 命令的操作系统沙箱配置。
#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct SandboxConfig {
    /// 是否启用沙箱；关闭后审核模式只保留逐条审批，不再隔离命令
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    /// 沙箱内网络策略
    #[serde(default)]
    pub network: SandboxNetworkMode,
    /// 工作区之外额外允许写入的目录，例如构建缓存；支持 `~/` 前缀
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub writable_roots: Vec<String>,
    /// 在内置凭据目录之外额外隐藏的路径；相对路径按工作区解析
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub deny_read: Vec<String>,
    /// 是否从沙箱命令环境中移除密钥类变量
    #[serde(default = "default_enabled")]
    pub scrub_env: bool,
    /// 清理密钥变量时仍然保留的变量名
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub env_passthrough: Vec<String>,
}

impl Default for SandboxConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            network: SandboxNetworkMode::Deny,
            writable_roots: Vec::new(),
            deny_read: Vec::new(),
            scrub_env: true,
            env_passthrough: Vec::new(),
        }
    }
}

/// 【沙箱配置】【默认开关】新旧配置缺省时启用沙箱与环境清理。
/// @returns true
fn default_enabled() -> bool {
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 验证旧配置缺少 sandbox 段时使用安全默认值。
    #[test]
    fn legacy_config_defaults_to_enabled_offline_sandbox() {
        let config: SandboxConfig = serde_json::from_str("{}").unwrap();
        assert_eq!(config, SandboxConfig::default());
        assert!(config.enabled);
        assert_eq!(config.network, SandboxNetworkMode::Deny);
        assert!(config.scrub_env);
    }

    /// 验证网络策略字符串解析与序列化一致。
    #[test]
    fn network_mode_round_trips() {
        let config: SandboxConfig =
            serde_json::from_str(r#"{"network":"allow","writable_roots":["~/.cargo"]}"#).unwrap();
        assert_eq!(config.network, SandboxNetworkMode::Allow);
        assert_eq!(
            SandboxNetworkMode::parse_or_default("allow").as_str(),
            "allow"
        );
        assert_eq!(
            SandboxNetworkMode::parse_or_default("x"),
            SandboxNetworkMode::Deny
        );
        let value = serde_json::to_value(&config).unwrap();
        assert_eq!(value["network"], "allow");
        assert_eq!(value["writable_roots"][0], "~/.cargo");
    }
}
