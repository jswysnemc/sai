/// Agent 单轮运行模式。
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum AgentMode {
    Yolo,
    Audited,
    AutoAudit,
    CodeModel,
    Plan,
}

impl AgentMode {
    /// 解析外部传入的模式名称。
    ///
    /// 参数:
    /// - `value`: 可选模式文本；缺省为 YOLO
    ///
    /// 返回:
    /// - 合法 Agent 模式
    pub(crate) fn parse(value: Option<&str>) -> anyhow::Result<Self> {
        match value.unwrap_or("yolo").trim().to_ascii_lowercase().as_str() {
            "plan" => Ok(Self::Plan),
            "audited" | "audit" => Ok(Self::Audited),
            "auto_audit" | "auto-audit" | "auto" => Ok(Self::AutoAudit),
            "code-model" | "code_model" | "codemode" => Ok(Self::CodeModel),
            "yolo" | "" => Ok(Self::Yolo),
            value => anyhow::bail!("unsupported run mode: {value}"),
        }
    }

    /// 返回终端界面使用的模式标签。
    ///
    /// 返回:
    /// - 稳定的大写模式名称
    pub fn label(self) -> &'static str {
        match self {
            Self::Yolo => "YOLO",
            Self::Audited => "AUDIT",
            Self::AutoAudit => "AUTO-AUDIT",
            Self::CodeModel => "CODE-MODEL",
            Self::Plan => "PLAN",
        }
    }

    /// 返回状态标签使用的稳定模式名称。
    ///
    /// 返回:
    /// - 小写模式标识
    pub(crate) fn key(self) -> &'static str {
        match self {
            Self::Yolo => "yolo",
            Self::Audited => "audited",
            Self::AutoAudit => "auto-audit",
            Self::CodeModel => "code-model",
            Self::Plan => "plan",
        }
    }

    /// 返回工具注册表使用的权限策略模式。
    ///
    /// 返回:
    /// - 与当前 Agent 模式一致的权限策略
    pub(crate) fn permission_profile_mode(self) -> crate::permission::PermissionProfileMode {
        match self {
            Self::Yolo => crate::permission::PermissionProfileMode::Yolo,
            Self::Audited => crate::permission::PermissionProfileMode::Audited,
            Self::AutoAudit => crate::permission::PermissionProfileMode::AutoAudit,
            Self::CodeModel => crate::permission::PermissionProfileMode::CodeModel,
            Self::Plan => crate::permission::PermissionProfileMode::Plan,
        }
    }

    /// 返回当前模式追加到系统提示词中的约束说明。
    ///
    /// 返回:
    /// - 对应模式的静态提示词
    pub(crate) fn reminder(self) -> &'static str {
        match self {
            Self::Yolo => crate::prompts::YOLO_REMINDER,
            Self::Audited => crate::prompts::AUDITED_REMINDER,
            Self::AutoAudit => crate::prompts::AUTO_AUDIT_REMINDER,
            Self::CodeModel => "<mode name=\"code-model\">Use codemode to execute JavaScript and compose tools.<name>(args). Discover tools with searchTools or describeTool. Direct tool calls are unavailable. Each nested tool retains normal permission checks and may require human approval. Await every tool call and emit only useful results.</mode>",
            Self::Plan => crate::prompts::PLAN_REMINDER,
        }
    }

    /// 是否需要权限审计（人工或自动）。
    ///
    /// 返回:
    /// - Audited / AutoAudit / CodeModel 为 true
    #[allow(dead_code)]
    pub(crate) fn needs_permission_audit(self) -> bool {
        matches!(self, Self::Audited | Self::AutoAudit | Self::CodeModel)
    }

    /// 编码为原子状态值。
    pub(crate) fn as_u8(self) -> u8 {
        match self {
            Self::Yolo => 0,
            Self::Audited => 1,
            Self::AutoAudit => 2,
            Self::Plan => 3,
            Self::CodeModel => 4,
        }
    }

    /// 从原子状态值解码。
    pub(crate) fn from_u8(value: u8) -> Self {
        match value {
            1 => Self::Audited,
            2 => Self::AutoAudit,
            3 => Self::Plan,
            4 => Self::CodeModel,
            _ => Self::Yolo,
        }
    }

    /// 【会话模式】【快捷切换】返回 Shift+Tab 的下一模式；计划模式保持到显式退出
    /// 参数: self 为当前模式；返回: 下一会话模式
    pub(crate) fn next(self) -> Self {
        match self {
            Self::Yolo => Self::Audited,
            Self::Audited => Self::AutoAudit,
            Self::AutoAudit => Self::CodeModel,
            Self::CodeModel => Self::Yolo,
            Self::Plan => Self::Plan,
        }
    }
}

impl From<crate::config::DefaultPermissionMode> for AgentMode {
    fn from(value: crate::config::DefaultPermissionMode) -> Self {
        match value {
            crate::config::DefaultPermissionMode::Yolo => Self::Yolo,
            crate::config::DefaultPermissionMode::Audited => Self::Audited,
            crate::config::DefaultPermissionMode::AutoAudit => Self::AutoAudit,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 验证各模式提示可识别且保持紧凑。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 无
    #[test]
    fn mode_prompts_are_compact_and_identifiable() {
        for (mode, name, max_chars) in [
            (AgentMode::Yolo, "yolo", 500),
            (AgentMode::Audited, "audited", 500),
            (AgentMode::AutoAudit, "auto-audit", 400),
            (AgentMode::Plan, "plan", 600),
            (AgentMode::CodeModel, "code-model", 600),
        ] {
            let prompt = mode.reminder();
            assert!(prompt.contains(&format!("name=\"{name}\"")));
            assert!(prompt.chars().count() <= max_chars);
        }
    }

    /// 【会话模式】【切换回归】校验快捷切换顺序、原子表示及权限表示一致
    /// 参数: 无；返回: 无
    #[test]
    fn mode_cycle_and_atomic_encoding_include_code_model() {
        let mut mode = AgentMode::Yolo;
        for expected in [
            AgentMode::Audited,
            AgentMode::AutoAudit,
            AgentMode::CodeModel,
            AgentMode::Yolo,
        ] {
            mode = mode.next();
            assert_eq!(mode, expected);
            assert_eq!(AgentMode::from_u8(mode.as_u8()), mode);
            assert_eq!(mode.permission_profile_mode().as_u8(), mode.as_u8());
        }
        assert_eq!(
            AgentMode::parse(Some("code-model")).unwrap(),
            AgentMode::CodeModel
        );
    }
}
