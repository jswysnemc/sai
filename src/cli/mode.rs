use super::{AgentMode, AppConfig, Cli, PermissionSurface, Result, SaiPaths};

/// 命令行分别覆盖权限策略和工具方式，未指定权限时沿用配置。
#[derive(Clone, Copy, Default)]
pub(super) struct ModeOverride {
    pub(super) permission: Option<AgentMode>,
    pub(super) codemode: bool,
}

/// 【会话模式】【命令行覆盖】参数 cli 为已解析参数；返回独立覆盖项，均未指定时为空。
pub(super) fn cli_mode_override(cli: &Cli) -> Option<ModeOverride> {
    let permission = if cli.plan {
        Some(AgentMode::Plan)
    } else if cli.audited {
        Some(AgentMode::Audited)
    } else if cli.auto_audit {
        Some(AgentMode::AutoAudit)
    } else if cli.yolo {
        Some(AgentMode::Yolo)
    } else {
        None
    };
    (permission.is_some() || cli.code_model).then_some(ModeOverride {
        permission,
        codemode: cli.code_model,
    })
}

/// 【会话模式】【启动解析】参数为路径、独立覆盖项和入口；返回合并默认设置后的完整模式。
pub(super) fn resolve_agent_mode(
    paths: &SaiPaths,
    overrides: Option<ModeOverride>,
    surface: PermissionSurface,
) -> Result<AgentMode> {
    let config = AppConfig::load_or_default(paths)?;
    let overrides = overrides.unwrap_or_default();
    let permission = overrides.permission.unwrap_or_else(|| match surface {
        PermissionSurface::Tui => config.permission.tui_mode().into(),
        PermissionSurface::Cli => config.permission.cli_mode().into(),
    });
    Ok(permission
        .with_codemode(overrides.codemode || config.codemode.mode != crate::config::CodeMode::Off))
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    /// 【会话模式】【参数组合】无参数；验证脚本标志与每种权限参数均可共存。
    #[test]
    fn code_model_accepts_independent_permission_flags() {
        for (flag, expected) in [
            ("--yolo", AgentMode::Yolo),
            ("--audited", AgentMode::Audited),
            ("--auto-audit", AgentMode::AutoAudit),
            ("--plan", AgentMode::Plan),
        ] {
            let cli = Cli::try_parse_from(["sai", flag, "--code-model"]).unwrap();
            let overrides = cli_mode_override(&cli).unwrap();
            assert_eq!(overrides.permission, Some(expected));
            assert!(overrides.codemode);
        }
        let cli = Cli::try_parse_from(["sai", "--code-model"]).unwrap();
        assert!(cli_mode_override(&cli).unwrap().permission.is_none());
    }
}
