use super::*;

/// 【终端命令】【屏幕策略】描述斜杠命令在全屏与内联视图下的画布切换行为。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum CommandScreenPolicy {
    /// 沿用当前受管画布直接执行，无需离开全屏或暂停主画面
    StayCurrent,
    /// 命令自身接管嵌套备用屏或居中浮层，全屏下暂停当前帧并在结束后重绘全屏
    AlternateOverlay,
    /// 命令依赖主屏预留滚动行展示内联选择器，全屏下需暂时回退主屏并在结束后恢复
    InlinePrompt,
}

/// 【终端命令】【策略分类】根据用户输入的斜杠命令判定所需的屏幕交接策略。
///
/// 参数:
/// - `input`: 已去除首尾空白的命令输入
///
/// 返回:
/// - 对应的屏幕交接策略
pub(super) fn classify_command_screen(input: &str) -> CommandScreenPolicy {
    let trimmed = input.trim();
    if !trimmed.starts_with('/') || session_commands::uses_context_panel(trimmed) {
        return CommandScreenPolicy::StayCurrent;
    }
    if trimmed.eq_ignore_ascii_case("/providers") {
        return CommandScreenPolicy::InlinePrompt;
    }
    if let Some(rest) = repl_command_rest(trimmed, "/thinking")
        .or_else(|| repl_command_rest(trimmed, "/think"))
    {
        return if rest.trim().is_empty() {
            CommandScreenPolicy::InlinePrompt
        } else {
            CommandScreenPolicy::StayCurrent
        };
    }
    if trimmed.eq_ignore_ascii_case("/config")
        || trimmed.eq_ignore_ascii_case("/ps")
        || group_commands::opens_group_overlay(trimmed)
    {
        return CommandScreenPolicy::AlternateOverlay;
    }
    match crate::control_commands::parse_control_command(
        trimmed,
        crate::control_commands::ControlSurface::Repl,
    ) {
        Ok(Some(command)) => match command {
            crate::control_commands::ControlCommand::Model { selection: None }
            | crate::control_commands::ControlCommand::Agent { selection: None }
            | crate::control_commands::ControlCommand::Tree { turn_id: None } => {
                CommandScreenPolicy::InlinePrompt
            }
            crate::control_commands::ControlCommand::Help
            | crate::control_commands::ControlCommand::Resume { id: None } => {
                CommandScreenPolicy::AlternateOverlay
            }
            _ => CommandScreenPolicy::StayCurrent,
        },
        _ => CommandScreenPolicy::StayCurrent,
    }
}

/// 【终端命令】【执行前交接】按命令策略暂停全屏帧或暂时回退主屏。
///
/// 参数:
/// - `input`: 用户提交的命令文本
/// - `runtime`: 终端运行期
///
/// 返回:
/// - 若因内联选择器暂时退出了全屏视图则返回 true，供命令结束后恢复全屏
pub(super) fn prepare_command_screen(input: &str, runtime: &mut ReplRuntime) -> Result<bool> {
    match classify_command_screen(input) {
        CommandScreenPolicy::StayCurrent => Ok(false),
        CommandScreenPolicy::AlternateOverlay => {
            prepare_alternate_overlay(runtime)?;
            Ok(false)
        }
        CommandScreenPolicy::InlinePrompt => {
            // 1. 【终端命令】【主屏预留】仅内联滚动列表暂时退出全屏，结束后按标志无损恢复
            let was_fullscreen = runtime.fullscreen_active();
            if was_fullscreen {
                runtime.leave_fullscreen()?;
            }
            Ok(was_fullscreen)
        }
    }
}

/// 【终端命令】【备用屏暂停】在进入嵌套备用屏或居中浮层前暂停实时动效并冲刷帧缓冲。
///
/// 参数:
/// - `runtime`: 终端运行期
///
/// 返回:
/// - 终端同步结果
pub(super) fn prepare_alternate_overlay(runtime: &mut ReplRuntime) -> Result<()> {
    runtime.pause_for_permission_prompt()
}

/// 【终端命令】【后台面板】在嵌套备用屏中打开后台任务管理器并在返回后恢复当前视图。
///
/// 参数:
/// - `paths`: Sai 路径集合
/// - `config`: 当前运行配置
/// - `runtime`: 终端运行期
///
/// 返回:
/// - 界面重绘与提示记录结果
pub(super) async fn run_background_manager_overlay(
    paths: &SaiPaths,
    config: &AppConfig,
    runtime: &mut ReplRuntime,
) -> Result<()> {
    // 1. 【终端命令】【任务管理】面板退出后整屏重绘，失败仅提示而不中断会话
    let result = run_repl_background_manager(paths, config).await;
    runtime.redraw()?;
    if let Err(err) = result {
        runtime.record_meta(err.to_string())?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{classify_command_screen, CommandScreenPolicy};

    /// 【终端命令测试】【策略回归】验证全屏备用屏命令、直接参数命令与内联选择器命令精确分流。
    #[test]
    fn slash_commands_select_minimal_screen_transitions() {
        for overlay in ["/config", "/CONFIG", "/resume", "/ps", "/help"] {
            assert_eq!(
                classify_command_screen(overlay),
                CommandScreenPolicy::AlternateOverlay,
                "{overlay}"
            );
        }
        for inline in ["/model", "/providers", "/agent", "/tree", "/thinking", "/think"] {
            assert_eq!(
                classify_command_screen(inline),
                CommandScreenPolicy::InlinePrompt,
                "{inline}"
            );
        }
        for stay in [
            "/thinking high",
            "/think auto",
            "/model 1",
            "/agent 2",
            "/tree root",
            "/resume session-1",
            "/context",
            "/context edit",
            "/new",
            "/clear",
            "/sandbox",
            "/yolo",
            "/exit",
            "ordinary message",
        ] {
            assert_eq!(
                classify_command_screen(stay),
                CommandScreenPolicy::StayCurrent,
                "{stay}"
            );
        }
    }
}
