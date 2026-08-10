use crate::agent::{Agent, AgentMode};
use crate::cli::repl_runtime::ReplRuntime;
use crate::cli::repl_turn::execute_repl_turn;
use crate::cli::{build_repl_tool_registry, stream_render_options};
use crate::config::AppConfig;
use crate::paths::SaiPaths;
use crate::{clipboard, render};
use anyhow::Result;

/// 将运行中 Shift+Tab 切换的模式应用到主循环。
///
/// 参数:
/// - `runtime`: TUI 运行期
/// - `mode`: 主循环模式
///
/// 返回:
/// - 无
pub(super) fn apply_stream_mode(runtime: &ReplRuntime, mode: &mut AgentMode) {
    if let Some(next) = runtime.stream_draft().mode {
        *mode = next;
    }
}

/// 取出流式阶段残留草稿作为下一轮预填内容。
///
/// 参数:
/// - `runtime`: TUI 运行期
///
/// 返回:
/// - 非空草稿文本与其剪贴板附件
pub(super) fn take_stream_draft_prefill(
    runtime: &mut ReplRuntime,
) -> Option<(String, crate::cli::repl_clipboard::ReplClipboardState)> {
    let text = runtime.stream_draft().text.trim().to_string();
    if text.is_empty() {
        return None;
    }
    let mode = runtime.stream_draft().mode;
    let draft = runtime.stream_draft_mut();
    // 附件随文本一起交还输入框，否则占位符会退化成字面文本
    let clipboard = std::mem::take(&mut draft.clipboard);
    *draft = Default::default();
    draft.mode = mode;
    Some((text, clipboard))
}

/// 依次执行运行期间 Tab 入队的用户消息。
///
/// 参数:
/// - `paths`: Sai 路径
/// - `config`: 应用配置
/// - `agent`: 复用 Agent
/// - `runtime`: TUI 运行期
/// - `mode`: 当前模式
/// - `input_history`: 输入历史
/// - `reasoning_mode`: 推理显示模式
/// - `tool_call_mode`: 工具显示模式
///
/// 返回:
/// - 队列执行结果
pub(super) async fn drain_submission_queue(
    paths: &SaiPaths,
    config: &AppConfig,
    agent: &mut Agent,
    runtime: &mut ReplRuntime,
    mode: &mut AgentMode,
    input_history: &mut Vec<String>,
    reasoning_mode: render::ReasoningDisplayMode,
    tool_call_mode: render::ToolCallDisplayMode,
) -> Result<()> {
    loop {
        let queued = runtime.take_submission_queue();
        if queued.is_empty() {
            break;
        }
        let mut pending = queued.into_iter();
        while let Some(item) = pending.next() {
            *mode = item.mode;
            let text = item.text.trim().to_string();
            // 斜杠命令不能作为聊天消息发给模型：/model 这类输入原先会被
            // 当成普通文本发出去。队列没有主循环的命令分发能力，
            // 此处交还输入框由用户确认执行，并丢弃其后的排队项保持顺序语义
            if is_queued_control_command(&text) {
                runtime.record_meta(crate::i18n::text(
                    "slash commands are not run from the queue; the text was returned to the input box",
                    "斜杠命令不会从队列执行，已交还输入框",
                ).to_string())?;
                restore_leftover_draft(runtime, Some(text));
                discard_remaining_queue(runtime, pending)?;
                return Ok(());
            }
            // 剪贴板附件在此还原：图片与长文本占位块换回真实内容
            let chat_input = item.clipboard.to_chat_input(&text);
            if chat_input.message.trim().is_empty() && chat_input.image_url.is_none() {
                continue;
            }
            input_history.push(text.clone());
            runtime.record_user(*mode, text.clone())?;
            if agent.installed_mode() != *mode {
                let registry = build_repl_tool_registry(config, paths, *mode)?;
                agent.switch_mode(*mode, registry)?;
            }
            agent.prepare_for_turn()?;
            let runner_submission = repl_runner_submission(
                chat_input,
                *mode,
                reasoning_mode,
                tool_call_mode,
                stream_render_options(config),
                false,
            );
            let outcome =
                execute_repl_turn(paths, config, agent, runtime, runner_submission).await?;
            apply_stream_mode(runtime, mode);
            if outcome.interrupted || outcome.result.is_err() {
                if let Err(error) = outcome.result {
                    runtime.record_meta(if outcome.interrupted {
                        super::super::repl_turn_failure::interrupted_failure_text(&error)
                    } else {
                        super::super::repl_turn_failure::turn_failure_text(&error)
                    })?;
                }
                restore_leftover_draft(runtime, outcome.leftover_draft);
                discard_remaining_queue(runtime, pending)?;
                return Ok(());
            }
            // 半截草稿只回填输入框等待用户提交，不再作为正式消息自动发送
            restore_leftover_draft(runtime, outcome.leftover_draft);
            // 本轮执行中新入队的项目由外层循环继续处理
        }
    }
    Ok(())
}

/// 判断排队文本是否为需要主循环执行的斜杠命令或 shell。
///
/// 队列只能发起模型轮次，无法执行控制命令；识别出来交还输入框，
/// 避免 `/model`、`!ls` 这类输入被当成聊天正文发给模型。
///
/// 参数:
/// - `text`: 已去除首尾空白的排队文本
///
/// 返回:
/// - 需要交还输入框时返回 true
fn is_queued_control_command(text: &str) -> bool {
    if text.starts_with('!') {
        return true;
    }
    matches!(
        crate::control_commands::parse_control_command(
            text,
            crate::control_commands::ControlSurface::Repl,
        ),
        Ok(Some(_))
    ) || super::super::repl_commands::is_repl_exit_command(text)
}

/// 丢弃中断或失败后剩余的排队项，并向用户说明数量。
///
/// 参数:
/// - `runtime`: TUI 运行期
/// - `remaining`: 尚未执行的排队项
///
/// 返回:
/// - 记录是否成功
fn discard_remaining_queue(
    runtime: &mut ReplRuntime,
    remaining: impl Iterator<Item = crate::cli::repl_runtime::QueuedSubmission>,
) -> Result<()> {
    let count = remaining.count();
    if count == 0 {
        return Ok(());
    }
    // 中断意图是停止一切，滞留的旧队列在下次对话后自动触发反而更意外；
    // 丢弃但必须留痕，用户可自行重新提交
    runtime.record_meta(if crate::i18n::is_zh() {
        format!("已丢弃 {count} 条未执行的排队消息")
    } else {
        format!("discarded {count} queued messages that had not run yet")
    })
}

/// 恢复中断或失败后留下的流式草稿。
///
/// 参数:
/// - `runtime`: TUI 运行期
/// - `draft`: 可选残留草稿
///
/// 返回:
/// - 无
fn restore_leftover_draft(runtime: &mut ReplRuntime, draft: Option<String>) {
    let Some(draft) = draft else {
        return;
    };
    let draft_state = runtime.stream_draft_mut();
    draft_state.text = draft.clone();
    draft_state.cursor = draft.chars().count();
}

/// 构造 REPL 单轮 runner submission。
///
/// 参数:
/// - `chat_input`: 剪贴板处理后的聊天输入
/// - `mode`: 当前 Agent 模式
/// - `reasoning_mode`: 推理内容显示方式
/// - `tool_call_mode`: 工具调用显示方式
/// - `render_options`: 流式渲染选项
/// - `goal_continuation`: 是否继续当前目标
///
/// 返回:
/// - runner submission
pub(super) fn repl_runner_submission(
    chat_input: clipboard::ClipboardChatInput,
    mode: AgentMode,
    reasoning_mode: render::ReasoningDisplayMode,
    tool_call_mode: render::ToolCallDisplayMode,
    render_options: render::StreamRenderOptions,
    goal_continuation: bool,
) -> crate::runner::RunnerSubmission {
    let mut user_input = match chat_input.image_url {
        Some(image_url) => crate::runner::UserInputSubmission::new(chat_input.message, mode)
            .with_image_url(image_url),
        None => crate::runner::UserInputSubmission::new(chat_input.message, mode),
    };
    if goal_continuation {
        user_input = user_input.with_goal_continuation();
    }
    crate::runner::RunnerSubmission::user_input(crate::runner::SubmissionSource::Repl, user_input)
        .with_render_policy(crate::runner::RenderPolicy::new(
            false,
            reasoning_mode,
            tool_call_mode,
            render_options,
        ))
        .with_final_summary(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 验证斜杠命令会被识别为需要主循环执行。
    ///
    /// 队列只能发起模型轮次，识别失败会让 /model 被当成聊天正文发出去。
    #[test]
    fn recognizes_slash_commands_in_the_queue() {
        assert!(is_queued_control_command("/model"));
        assert!(is_queued_control_command("/compact"));
        assert!(is_queued_control_command("/new 标题"));
    }

    /// 验证 shell 前缀同样交还输入框。
    #[test]
    fn recognizes_shell_prefix_in_the_queue() {
        assert!(is_queued_control_command("!ls -al"));
    }

    /// 验证退出命令不会被当作聊天消息发给模型。
    #[test]
    fn recognizes_exit_commands_in_the_queue() {
        assert!(is_queued_control_command("/exit"));
    }

    /// 验证普通消息与含斜杠的正文仍走模型轮次。
    ///
    /// 误判会让正常提问被拦下来，代价比漏判更高。
    #[test]
    fn keeps_ordinary_messages_in_the_queue() {
        assert!(!is_queued_control_command("继续实现这个功能"));
        assert!(!is_queued_control_command("路径是 src/main.rs"));
        assert!(!is_queued_control_command("/definitely-not-a-command"));
        assert!(!is_queued_control_command(""));
    }

    /// 验证 REPL 聊天输入会构造成 runner submission。
    #[test]
    fn repl_chat_input_builds_runner_submission() {
        let submission = repl_runner_submission(
            clipboard::ClipboardChatInput {
                message: "继续".to_string(),
                image_url: Some("data:image/png;base64,AAAA".to_string()),
            },
            AgentMode::Yolo,
            render::ReasoningDisplayMode::Summary,
            render::ToolCallDisplayMode::Summary,
            render::StreamRenderOptions::default(),
            false,
        );

        assert_eq!(submission.source, crate::runner::SubmissionSource::Repl);
        assert!(matches!(
            submission.kind,
            crate::runner::RunnerSubmissionKind::UserInput(crate::runner::UserInputSubmission {
                image_urls,
                ..
            }) if image_urls.len() == 1
        ));
    }
}
