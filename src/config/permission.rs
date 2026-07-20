use serde::{Deserialize, Serialize};

/// CLI 与 TUI 启动时采用的默认权限模式。
#[derive(Debug, Clone, Copy, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DefaultPermissionMode {
    Yolo,
    Audited,
    AutoAudit,
    Plan,
}

impl DefaultPermissionMode {
    /// 返回配置文件使用的稳定字符串。
    ///
    /// 返回:
    /// - `yolo`、`audited`、`auto_audit` 或 `plan`
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Yolo => "yolo",
            Self::Audited => "audited",
            Self::AutoAudit => "auto_audit",
            Self::Plan => "plan",
        }
    }

    /// 从配置界面字符串解析默认权限模式。
    ///
    /// 参数:
    /// - `value`: 配置字符串
    ///
    /// 返回:
    /// - 已识别模式；未知值回退到 YOLO
    pub fn parse_or_default(value: &str) -> Self {
        match value.trim() {
            "audited" | "audit" => Self::Audited,
            "auto_audit" | "auto-audit" | "auto" => Self::AutoAudit,
            "plan" => Self::Plan,
            _ => Self::Yolo,
        }
    }
}

impl Default for DefaultPermissionMode {
    fn default() -> Self {
        Self::Yolo
    }
}

/// 终端运行入口使用的权限配置；TUI 与 CLI 可分别设置。
#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct PermissionConfig {
    /// 兼容旧字段：当 tui_mode / cli_mode 缺省时作为共用默认值。
    #[serde(default)]
    pub default_mode: DefaultPermissionMode,
    /// TUI（交互 REPL）默认权限模式；缺省时回退 `default_mode`。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tui_mode: Option<DefaultPermissionMode>,
    /// CLI 单次命令（ask/tool 等）默认权限模式；缺省时回退 `default_mode`。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cli_mode: Option<DefaultPermissionMode>,
    /// 自动审核专用供应商；空则使用当前会话供应商。
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub auto_audit_provider_id: String,
    /// 自动审核专用模型；空则使用当前会话模型。
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub auto_audit_model: String,
}

impl Default for PermissionConfig {
    fn default() -> Self {
        Self {
            default_mode: DefaultPermissionMode::Yolo,
            tui_mode: None,
            cli_mode: None,
            auto_audit_provider_id: String::new(),
            auto_audit_model: String::new(),
        }
    }
}

impl PermissionConfig {
    /// 返回 TUI 使用的默认权限模式。
    pub fn tui_mode(&self) -> DefaultPermissionMode {
        self.tui_mode.unwrap_or(self.default_mode)
    }

    /// 返回 CLI 使用的默认权限模式。
    pub fn cli_mode(&self) -> DefaultPermissionMode {
        self.cli_mode.unwrap_or(self.default_mode)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn permission_config_defaults_to_yolo_for_legacy_configs() {
        let config: PermissionConfig = serde_json::from_str("{}").unwrap();
        assert_eq!(config.default_mode, DefaultPermissionMode::Yolo);
        assert_eq!(config.tui_mode(), DefaultPermissionMode::Yolo);
        assert_eq!(config.cli_mode(), DefaultPermissionMode::Yolo);
        assert!(config.auto_audit_provider_id.is_empty());
    }

    #[test]
    fn permission_config_supports_separate_tui_and_cli_modes() {
        let config: PermissionConfig = serde_json::from_str(
            r#"{"default_mode":"yolo","tui_mode":"audited","cli_mode":"plan"}"#,
        )
        .unwrap();
        assert_eq!(config.tui_mode(), DefaultPermissionMode::Audited);
        assert_eq!(config.cli_mode(), DefaultPermissionMode::Plan);

        let legacy: PermissionConfig =
            serde_json::from_str(r#"{"default_mode":"audited"}"#).unwrap();
        assert_eq!(legacy.tui_mode(), DefaultPermissionMode::Audited);
        assert_eq!(legacy.cli_mode(), DefaultPermissionMode::Audited);
    }

    #[test]
    fn parses_auto_audit_mode() {
        assert_eq!(
            DefaultPermissionMode::parse_or_default("auto_audit"),
            DefaultPermissionMode::AutoAudit
        );
        assert_eq!(
            DefaultPermissionMode::parse_or_default("auto"),
            DefaultPermissionMode::AutoAudit
        );
    }
}
