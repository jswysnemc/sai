/// 会话权限策略，与工具调用方式相互独立。
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum PermissionMode {
    Yolo,
    Audited,
    AutoAudit,
    Plan,
}

/// 工具调用方式，不改变会话权限。
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum ToolMode {
    Native,
    CodeModel,
    CodeModelOnly,
}

/// 会话模式由权限策略和工具调用方式组合而成。
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct AgentMode {
    permission: PermissionMode,
    tools: ToolMode,
}

#[allow(non_upper_case_globals)]
impl AgentMode {
    pub const Yolo: Self = Self::new(PermissionMode::Yolo, ToolMode::Native);
    pub const Audited: Self = Self::new(PermissionMode::Audited, ToolMode::Native);
    pub const AutoAudit: Self = Self::new(PermissionMode::AutoAudit, ToolMode::Native);
    pub const Plan: Self = Self::new(PermissionMode::Plan, ToolMode::Native);
    /// 旧会话的独立 code-model 值保留人工审核语义。
    pub const CodeModel: Self = Self::new(PermissionMode::Audited, ToolMode::CodeModel);

    /// 【会话模式】【组合】参数为权限策略和工具方式；返回完整会话模式。
    pub const fn new(permission: PermissionMode, tools: ToolMode) -> Self {
        Self { permission, tools }
    }

    /// 【会话模式】【权限读取】无参数；返回当前权限策略。
    pub fn permission(self) -> PermissionMode {
        self.permission
    }

    /// 【会话模式】【工具读取】无参数；返回是否通过脚本调用工具。
    pub fn uses_codemode(self) -> bool {
        self.tools != ToolMode::Native
    }

    /// 【会话模式】【脚本策略】无参数；返回混合、独占或关闭策略。
    pub(crate) fn codemode(self) -> crate::config::CodeMode {
        match self.tools {
            ToolMode::Native => crate::config::CodeMode::Off,
            ToolMode::CodeModel => crate::config::CodeMode::On,
            ToolMode::CodeModelOnly => crate::config::CodeMode::Only,
        }
    }

    /// 【会话模式】【脚本配置】参数 mode 为脚本策略；返回保留权限的完整模式。
    pub(crate) fn with_code_mode(self, mode: crate::config::CodeMode) -> Self {
        Self {
            tools: match mode {
                crate::config::CodeMode::Off => ToolMode::Native,
                crate::config::CodeMode::On => ToolMode::CodeModel,
                crate::config::CodeMode::Only => ToolMode::CodeModelOnly,
            },
            ..self
        }
    }

    /// 【会话模式】【权限切换】参数 permission 为新策略；返回保留工具方式的模式。
    pub fn with_permission(self, permission: PermissionMode) -> Self {
        Self { permission, ..self }
    }

    /// 【会话模式】【工具切换】参数 enabled 决定是否启用脚本；返回保留权限策略的模式。
    pub fn with_codemode(self, enabled: bool) -> Self {
        Self {
            tools: if enabled {
                ToolMode::CodeModel
            } else {
                ToolMode::Native
            },
            ..self
        }
    }

    /// 【会话模式】【协议解析】参数 value 为存储值；返回模式，旧 code-model 按人工审核读取。
    pub(crate) fn parse(value: Option<&str>) -> anyhow::Result<Self> {
        let value = value.unwrap_or("yolo").trim().to_ascii_lowercase();
        if matches!(value.as_str(), "code-model" | "code_model" | "codemode") {
            return Ok(Self::CodeModel);
        }
        let (permission, tools) = value.split_once('+').unwrap_or((&value, "native"));
        let permission = match permission {
            "yolo" | "" => PermissionMode::Yolo,
            "audited" | "audit" => PermissionMode::Audited,
            "auto_audit" | "auto-audit" | "auto" => PermissionMode::AutoAudit,
            "plan" => PermissionMode::Plan,
            _ => anyhow::bail!("unsupported run mode: {value}"),
        };
        let tools = match tools {
            "native" => ToolMode::Native,
            "code-model" | "code_model" | "codemode" => ToolMode::CodeModel,
            "code-model-only" => ToolMode::CodeModelOnly,
            _ => anyhow::bail!("unsupported tool mode: {tools}"),
        };
        Ok(Self::new(permission, tools))
    }

    /// 【会话模式】【入口默认值】参数为请求模式及配置策略；未显式指定工具方式时沿用配置，返回完整模式。
    pub(crate) fn parse_configured(
        value: Option<&str>,
        default: crate::config::CodeMode,
    ) -> anyhow::Result<Self> {
        let mode = Self::parse(value)?;
        let explicit = value.is_some_and(|value| {
            let value = value.trim().to_ascii_lowercase();
            value.contains('+')
                || matches!(value.as_str(), "code-model" | "code_model" | "codemode")
        });
        Ok(if explicit {
            mode
        } else {
            mode.with_code_mode(default)
        })
    }

    /// 【会话模式】【命令解析】参数 input 为完整命令；返回只更新指定维度的模式或空值。
    pub(crate) fn command(self, input: &str) -> Option<Self> {
        match input.trim().to_ascii_lowercase().as_str() {
            "/yolo" => Some(self.with_permission(PermissionMode::Yolo)),
            "/audit" => Some(self.with_permission(PermissionMode::Audited)),
            "/auto" | "/auto-audit" => Some(self.with_permission(PermissionMode::AutoAudit)),
            "/plan" => Some(self.with_permission(PermissionMode::Plan)),
            "/code-model" => Some(self.with_codemode(!self.uses_codemode())),
            "/code-model on" => Some(self.with_codemode(true)),
            "/code-model off" => Some(self.with_codemode(false)),
            "/code-model only" => Some(self.with_code_mode(crate::config::CodeMode::Only)),
            _ => None,
        }
    }

    /// 【会话模式】【显示名称】无参数；返回同时表达权限和工具方式的标签。
    pub fn label(self) -> &'static str {
        match (self.permission, self.tools) {
            (PermissionMode::Yolo, ToolMode::Native) => "YOLO",
            (PermissionMode::Audited, ToolMode::Native) => "AUDIT",
            (PermissionMode::AutoAudit, ToolMode::Native) => "AUTO-AUDIT",
            (PermissionMode::Plan, ToolMode::Native) => "PLAN",
            (PermissionMode::Yolo, ToolMode::CodeModel) => "YOLO + CODE-MODEL",
            (PermissionMode::Audited, ToolMode::CodeModel) => "AUDIT + CODE-MODEL",
            (PermissionMode::AutoAudit, ToolMode::CodeModel) => "AUTO-AUDIT + CODE-MODEL",
            (PermissionMode::Plan, ToolMode::CodeModel) => "PLAN + CODE-MODEL",
            (PermissionMode::Yolo, ToolMode::CodeModelOnly) => "YOLO + CODE-MODEL-ONLY",
            (PermissionMode::Audited, ToolMode::CodeModelOnly) => "AUDIT + CODE-MODEL-ONLY",
            (PermissionMode::AutoAudit, ToolMode::CodeModelOnly) => "AUTO-AUDIT + CODE-MODEL-ONLY",
            (PermissionMode::Plan, ToolMode::CodeModelOnly) => "PLAN + CODE-MODEL-ONLY",
        }
    }

    /// 【会话模式】【存储名称】无参数；返回可往返解析的稳定协议值。
    pub(crate) fn key(self) -> &'static str {
        match (self.permission, self.tools) {
            (PermissionMode::Yolo, ToolMode::Native) => "yolo",
            (PermissionMode::Audited, ToolMode::Native) => "audited",
            (PermissionMode::AutoAudit, ToolMode::Native) => "auto-audit",
            (PermissionMode::Plan, ToolMode::Native) => "plan",
            (PermissionMode::Yolo, ToolMode::CodeModel) => "yolo+code-model",
            (PermissionMode::Audited, ToolMode::CodeModel) => "audited+code-model",
            (PermissionMode::AutoAudit, ToolMode::CodeModel) => "auto-audit+code-model",
            (PermissionMode::Plan, ToolMode::CodeModel) => "plan+code-model",
            (PermissionMode::Yolo, ToolMode::CodeModelOnly) => "yolo+code-model-only",
            (PermissionMode::Audited, ToolMode::CodeModelOnly) => "audited+code-model-only",
            (PermissionMode::AutoAudit, ToolMode::CodeModelOnly) => "auto-audit+code-model-only",
            (PermissionMode::Plan, ToolMode::CodeModelOnly) => "plan+code-model-only",
        }
    }

    /// 【会话模式】【权限适配】无参数；仅将权限维度传给工具注册表。
    pub(crate) fn permission_profile_mode(self) -> crate::permission::PermissionProfileMode {
        match self.permission {
            PermissionMode::Yolo => crate::permission::PermissionProfileMode::Yolo,
            PermissionMode::Audited => crate::permission::PermissionProfileMode::Audited,
            PermissionMode::AutoAudit => crate::permission::PermissionProfileMode::AutoAudit,
            PermissionMode::Plan => crate::permission::PermissionProfileMode::Plan,
        }
    }

    /// 【会话模式】【提示约束】无参数；返回权限说明和可选的脚本工具说明。
    pub(crate) fn reminder(self) -> String {
        let permission = match self.permission {
            PermissionMode::Yolo => crate::prompts::YOLO_REMINDER,
            PermissionMode::Audited => crate::prompts::AUDITED_REMINDER,
            PermissionMode::AutoAudit => crate::prompts::AUTO_AUDIT_REMINDER,
            PermissionMode::Plan => crate::prompts::PLAN_REMINDER,
        };
        if self.uses_codemode() {
            let strategy = if self.codemode() == crate::config::CodeMode::Only {
                "Execute business tools through codemode. Native load and request_capability remain available for discovery and loading."
            } else {
                "Use native calls for simple operations. Use codemode for loops, dependent operations or filtering intermediate results. Loaded deferred tools may be called through invoke_tool or codemode."
            };
            format!("{permission}\n<mode name=\"code-model\">{strategy} Both paths share the existing tool descriptions and argument schemas. Load deferred tools before using them; request_capability follows Jev routing when enabled. In scripts use tools[name](args), await every call and emit only useful results. Nested calls retain the same permissions; completed side effects are not rolled back.</mode>")
        } else {
            permission.into()
        }
    }

    /// 【会话模式】【审核判断】无参数；返回当前权限策略是否需要人工或自动审核。
    #[allow(dead_code)]
    pub(crate) fn needs_permission_audit(self) -> bool {
        matches!(
            self.permission,
            PermissionMode::Audited | PermissionMode::AutoAudit
        )
    }

    /// 【会话模式】【快捷切换】无参数；循环权限并保留工具方式，计划模式保持不变。
    pub(crate) fn next(self) -> Self {
        self.with_permission(match self.permission {
            PermissionMode::Yolo => PermissionMode::Audited,
            PermissionMode::Audited => PermissionMode::AutoAudit,
            PermissionMode::AutoAudit => PermissionMode::Yolo,
            PermissionMode::Plan => PermissionMode::Plan,
        })
    }
}

impl From<crate::config::DefaultPermissionMode> for AgentMode {
    /// 【会话模式】【默认权限】参数 value 为配置权限；返回使用直接工具的会话模式。
    fn from(value: crate::config::DefaultPermissionMode) -> Self {
        match value {
            crate::config::DefaultPermissionMode::Yolo => Self::Yolo,
            crate::config::DefaultPermissionMode::Audited => Self::Audited,
            crate::config::DefaultPermissionMode::AutoAudit => Self::AutoAudit,
        }
    }
}
