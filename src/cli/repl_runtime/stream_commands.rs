//! 模型运行期间可立即执行的斜杠命令。
//!
//! 轮次进行中 `&mut Agent` 被 `execute_repl_turn` 独占，交互选择器也会
//! 占住事件循环，所以只有不触碰这两者的命令能在这里同步执行；其余命令
//! 要么在斜杠面板里置灰，要么等本轮结束后由主循环分发。

use super::ReplRuntime;
use crate::agent::AgentMode;
use crate::cli::SaiPaths;
use crate::control_commands::{ControlCommand, ControlSurface};
use anyhow::Result;

/// 流式按键处理的结果。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::cli) enum StreamInputAction {
    /// 继续当前轮次
    Continue,
    /// 中断当前轮次（Ctrl+C）
    Interrupt,
    /// 中断当前轮次并退出 REPL
    Exit,
}

/// 运行期间立即执行命令所需的上下文。
///
/// Agent 在轮次中被独占借用，这里只保存命令自己能取到的东西，
/// 因此必须在建 chat future 之前抓取。
pub(in crate::cli) struct StreamCommandContext {
    paths: SaiPaths,
    /// 子智能体作用域键：子智能体按父会话隔离
    owner_key: String,
    /// 本轮启动时的权限模式
    turn_mode: AgentMode,
}

impl StreamCommandContext {
    /// 在轮次开始、Agent 尚未被独占前抓取上下文。
    ///
    /// 参数:
    /// - `paths`: Sai 路径
    /// - `owner_key`: 当前会话的子智能体作用域键
    /// - `turn_mode`: 本轮权限模式
    ///
    /// 返回:
    /// - 立即执行命令上下文
    pub(in crate::cli) fn capture(
        paths: &SaiPaths,
        owner_key: String,
        turn_mode: AgentMode,
    ) -> Self {
        Self {
            paths: paths.clone(),
            owner_key,
            turn_mode,
        }
    }
}

/// 执行一条运行期间可立即执行的命令。
///
/// 共同点是不触碰 `&mut Agent`、不弹交互选择器、不 await，因此可以在
/// 流式 tick 的同步路径上跑完。副作用限于写 transcript、写子智能体
/// 消息队列（`/msg`）、改会话压缩策略（`/context` 带参数）与切权限模式。
///
/// 参数:
/// - `runtime`: 当前 REPL 运行期
/// - `ctx`: 轮次开始时抓取的上下文
/// - `text`: 命令原文
///
/// 返回:
/// - 操作结果
pub(in crate::cli) fn run_immediate_stream_command(
    runtime: &mut ReplRuntime,
    ctx: &StreamCommandContext,
    text: &str,
) -> Result<()> {
    let input = text.trim();
    match crate::control_commands::parse_control_command(input, ControlSurface::Repl) {
        Ok(Some(ControlCommand::Help)) => {
            // 帮助不走浮层：浮层内的阻塞读键会让 chat future 停止被 poll，
            // 模型流与工具子进程管道无人读取。整段作为 meta 进入 transcript
            runtime.record_meta(crate::control_commands::help_text(
                ControlSurface::Repl,
                runtime.paste_image_key(),
            ))
        }
        Ok(Some(ControlCommand::Context { update })) => {
            let info = crate::control_commands::context_info_for_mode_with_update(
                &ctx.paths,
                ctx.turn_mode,
                update,
            );
            runtime.record_meta(info.unwrap_or_else(|error| error.to_string()))
        }
        Ok(Some(ControlCommand::Subagents)) => runtime.record_meta(
            crate::cli::repl::subagent_commands::format_subagent_list(&ctx.owner_key),
        ),
        Ok(Some(ControlCommand::SubagentMessage { target, message })) => {
            let viewing = runtime.viewing_subagent_id();
            let notice = crate::cli::repl::subagent_commands::deliver_subagent_message(
                &ctx.owner_key,
                target.as_deref(),
                &message,
                viewing.as_deref(),
            );
            runtime.record_meta(notice.unwrap_or_else(|error| error.to_string()))
        }
        Ok(Some(ControlCommand::Rename { title })) => {
            match crate::control_commands::rename_current_session(&ctx.paths, &title) {
                Ok(message) => {
                    // 底栏不再展示会话标题，改名无需同步 chrome
                    runtime.record_meta(message)?;
                    runtime.redraw_stream_composer()
                }
                Err(error) => runtime.record_meta(error.to_string()),
            }
        }
        Ok(Some(_)) => Ok(()),
        Err(error) => runtime.record_meta(error.to_string()),
        Ok(None) => match stream_mode_switch(input) {
            // 权限模式切换等价于 Shift+Tab 热切换：只改运行期草稿模式，
            // 由共享的 live 句柄通知 Agent，不需要 &mut Agent
            Some(mode) => {
                runtime.stream_draft_mut().mode = Some(mode);
                let _ = runtime.apply_stream_mode_live(ctx.turn_mode);
                runtime.record_meta(mode_notice(mode))
            }
            None => Ok(()),
        },
    }
}

/// 返回输入对应的权限模式切换目标。
///
/// 参数:
/// - `input`: 已去除首尾空白的命令原文
///
/// 返回:
/// - 模式切换命令的目标模式；其它输入为空
fn stream_mode_switch(input: &str) -> Option<AgentMode> {
    if input.eq_ignore_ascii_case("/plan") {
        return Some(AgentMode::Plan);
    }
    if input.eq_ignore_ascii_case("/audit") {
        return Some(AgentMode::Audited);
    }
    if input.eq_ignore_ascii_case("/yolo") {
        return Some(AgentMode::Yolo);
    }
    if input.eq_ignore_ascii_case("/auto") || input.eq_ignore_ascii_case("/auto-audit") {
        return Some(AgentMode::AutoAudit);
    }
    None
}

/// 生成模式切换的提示文本。
///
/// 参数:
/// - `mode`: 切换后的模式
///
/// 返回:
/// - 中英双语提示
fn mode_notice(mode: AgentMode) -> String {
    if crate::i18n::is_zh() {
        format!("模式：{}", mode.label())
    } else {
        format!("mode: {}", mode.label())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 模式切换命令解析到对应模式。
    #[test]
    fn mode_switch_commands_map_to_modes() {
        assert_eq!(stream_mode_switch("/plan"), Some(AgentMode::Plan));
        assert_eq!(stream_mode_switch("/AUDIT"), Some(AgentMode::Audited));
        assert_eq!(stream_mode_switch("/yolo"), Some(AgentMode::Yolo));
        assert_eq!(stream_mode_switch("/auto"), Some(AgentMode::AutoAudit));
        assert_eq!(
            stream_mode_switch("/auto-audit"),
            Some(AgentMode::AutoAudit)
        );
        assert_eq!(stream_mode_switch("/model"), None);
        assert_eq!(stream_mode_switch("hello"), None);
    }

    /// 模式提示带上模式名，用户能确认切到了哪个。
    #[test]
    fn mode_notice_names_the_mode() {
        let notice = mode_notice(AgentMode::Plan);
        assert!(
            notice.contains(AgentMode::Plan.label()),
            "notice should name the mode: {notice}"
        );
    }
}
