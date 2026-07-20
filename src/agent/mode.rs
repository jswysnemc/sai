/// Agent 单轮运行模式。
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum AgentMode {
    Yolo,
    Audited,
    AutoAudit,
    Plan,
}

impl AgentMode {
    /// 返回终端界面使用的模式标签。
    ///
    /// 返回:
    /// - 稳定的大写模式名称
    pub fn label(self) -> &'static str {
        match self {
            Self::Yolo => "YOLO",
            Self::Audited => "AUDIT",
            Self::AutoAudit => "AUTO",
            Self::Plan => "PLAN",
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
            Self::Plan => crate::permission::PermissionProfileMode::Plan,
        }
    }

    /// 返回当前模式追加到系统提示词中的约束说明。
    ///
    /// 返回:
    /// - 对应模式的静态提示词
    pub(super) fn reminder(self) -> &'static str {
        match self {
            Self::Yolo => crate::prompts::YOLO_REMINDER,
            Self::Audited => crate::prompts::AUDITED_REMINDER,
            Self::AutoAudit => crate::prompts::AUTO_AUDIT_REMINDER,
            Self::Plan => crate::prompts::PLAN_REMINDER,
        }
    }

    /// 是否需要权限审计（人工或自动）。
    ///
    /// 返回:
    /// - Audited / AutoAudit 为 true
    #[allow(dead_code)]
    pub(crate) fn needs_permission_audit(self) -> bool {
        matches!(self, Self::Audited | Self::AutoAudit)
    }
}

impl From<crate::config::DefaultPermissionMode> for AgentMode {
    fn from(value: crate::config::DefaultPermissionMode) -> Self {
        match value {
            crate::config::DefaultPermissionMode::Yolo => Self::Yolo,
            crate::config::DefaultPermissionMode::Audited => Self::Audited,
            crate::config::DefaultPermissionMode::AutoAudit => Self::AutoAudit,
            crate::config::DefaultPermissionMode::Plan => Self::Plan,
        }
    }
}
