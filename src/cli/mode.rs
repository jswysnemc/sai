use super::{AgentMode, AppConfig, Cli, PermissionSurface, Result, SaiPaths};

/// 命令行分别覆盖权限策略和工具方式，未指定权限时沿用配置。
#[derive(Clone, Copy, Default)]
pub(super) struct ModeOverride {
    pub(super) permission: Option<AgentMode>,
    pub(super) codemode: Option<crate::config::CodeMode>,
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
    let codemode = if cli.code_model_only {
        Some(crate::config::CodeMode::Only)
    } else if cli.no_code_model {
        Some(crate::config::CodeMode::Off)
    } else if cli.code_model {
        Some(crate::config::CodeMode::On)
    } else {
        None
    };
    (permission.is_some() || codemode.is_some()).then_some(ModeOverride {
        permission,
        codemode,
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
    Ok(permission.with_code_mode(overrides.codemode.unwrap_or(config.codemode.mode)))
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
            assert_eq!(overrides.codemode, Some(crate::config::CodeMode::On));
        }
        let cli = Cli::try_parse_from(["sai", "--code-model"]).unwrap();
        assert!(cli_mode_override(&cli).unwrap().permission.is_none());
    }

    /// 【会话模式】【启动覆盖】无参数；验证默认混合、配置独占和显式关闭均保留权限。
    #[test]
    fn codemode_defaults_and_explicit_cli_overrides() {
        use crate::config::CodeMode;
        let root = tempfile::tempdir().unwrap();
        let paths = SaiPaths::for_tests(root.path());
        let default = resolve_agent_mode(&paths, None, PermissionSurface::Tui).unwrap();
        assert_eq!(default.codemode(), CodeMode::On);
        for (flag, expected) in [
            ("--code-model", CodeMode::On),
            ("--code-model-only", CodeMode::Only),
            ("--no-code-model", CodeMode::Off),
        ] {
            let cli = Cli::try_parse_from(["sai", "--audited", flag]).unwrap();
            let resolved =
                resolve_agent_mode(&paths, cli_mode_override(&cli), PermissionSurface::Cli)
                    .unwrap();
            assert_eq!(resolved.codemode(), expected);
            assert_eq!(resolved.permission(), crate::agent::PermissionMode::Audited);
        }
        assert!(Cli::try_parse_from(["sai", "--code-model", "--no-code-model"]).is_err());
        assert!(Cli::try_parse_from(["sai", "--code-model", "--code-model-only"]).is_err());
    }
}
