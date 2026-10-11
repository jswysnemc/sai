use crate::agent::{Agent, AgentMode};
use crate::cli::repl_runtime::{QueuedSubmission, ReplRuntime, StreamComposerDraft};
use crate::cli::repl_turn::{execute_repl_turn, ReplTurnOutcome};
use crate::cli::repl_turn_context::ReplTurnContext;
use crate::cli::{build_repl_tool_registry, stream_render_options};
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
/// - `context`: 应用路径、配置和当前会话的所有权键
/// - `agent`: 复用 Agent
/// - `runtime`: TUI 运行期
/// - `mode`: 当前模式
/// - `input_history`: 输入历史
/// - `reasoning_mode`: 推理显示模式
/// - `tool_call_mode`: 工具显示模式
///
/// 返回:
/// - 队列执行结果，以及是否收到退出请求
pub(super) async fn drain_submission_queue(
    context: ReplTurnContext<'_>,
    agent: &mut Agent,
    runtime: &mut ReplRuntime,
    mode: &mut AgentMode,
    input_history: &mut Vec<crate::state::input_history::InputHistoryEntry>,
    reasoning_mode: render::ReasoningDisplayMode,
    tool_call_mode: render::ToolCallDisplayMode,
) -> Result<bool> {
    // 1. 【终端队列】【逐条执行】尚未开始的消息始终留在运行期，错误返回不会销毁局部批次
    while let Some((position, item)) = runtime.take_next_turn_submission() {
        *mode = item.mode;
        if is_queued_control_command(item.text.trim()) {
            restore_control_submission(runtime, position, item)?;
            notify_preserved_queue(runtime)?;
            return Ok(false);
        }
        let outcome = execute_queued_turn(
            &context,
            agent,
            runtime,
            &item,
            input_history,
            reasoning_mode,
            tool_call_mode,
        )
        .await;
        let outcome = match outcome {
            Ok(Some(outcome)) => outcome,
            Ok(None) => continue,
            Err(error) => {
                // 2. 【终端队列】【启动恢复】准备或启动失败时连当前条目一起恢复，先恢复数据再绘制错误
                runtime.restore_queued_submission(position, item);
                runtime.pause_submission_queue();
                runtime
                    .record_failure(super::super::repl_turn_failure::turn_failure_text(&error))?;
                notify_preserved_queue(runtime)?;
                return Ok(false);
            }
        };
        apply_stream_mode(runtime, mode);
        restore_leftover_draft(runtime, outcome.leftover_draft);
        if outcome.exit_requested {
            return Ok(true);
        }
        if outcome.interrupted || outcome.result.is_err() {
            if let Err(error) = outcome.result {
                runtime.record_failure(if outcome.interrupted {
                    super::super::repl_turn_failure::interrupted_failure_text(&error)
                } else {
                    super::super::repl_turn_failure::turn_failure_text(&error)
                })?;
            }
            // 3. 【终端队列】【失败保留】停止自动消费，不清除后续消息，也不覆盖尚未提交的草稿
            notify_preserved_queue(runtime)?;
            return Ok(false);
        }
    }
    Ok(false)
}

/// 【终端队列】【单轮执行】还原附件、准备会话并执行一条排队提交。
///
/// 参数:
/// - `context`: 应用路径、配置和会话所有权键
/// - `agent`: 当前智能体
/// - `runtime`: 终端运行期
/// - `item`: 保留原始文本和附件的待执行消息
/// - `input_history`: 输入历史
/// - `reasoning_mode`: 推理展示方式
/// - `tool_call_mode`: 工具展示方式
///
/// 返回: 单轮结果；空消息返回空值，启动失败返回错误供调用方恢复条目
async fn execute_queued_turn(
    context: &ReplTurnContext<'_>,
    agent: &mut Agent,
    runtime: &mut ReplRuntime,
    item: &QueuedSubmission,
    input_history: &mut Vec<crate::state::input_history::InputHistoryEntry>,
    reasoning_mode: render::ReasoningDisplayMode,
    tool_call_mode: render::ToolCallDisplayMode,
) -> Result<Option<ReplTurnOutcome>> {
    let ReplTurnContext {
        paths,
        config,
        owner_key,
    } = *context;
    let text = item.text.trim();
    let mut chat_input = item.clipboard.to_chat_input(text);
    chat_input.message =
        crate::cli::repl_mentions::expand_skill_mentions(&chat_input.message, config, paths);
    if chat_input.message.trim().is_empty() && chat_input.image_url.is_none() {
        return Ok(None);
    }
    // 1. 【终端队列】【单轮执行】准备成功后再记录提交，启动失败时原消息仍可恢复
    if agent.installed_mode() != item.mode {
        let registry = build_repl_tool_registry(config, paths, item.mode)?;
        agent.switch_mode(item.mode, registry)?;
    }
    agent.prepare_for_turn()?;
    crate::cli::repl_input::history::remember_history(
        paths,
        input_history,
        item.clipboard.history_entry(&item.text),
    );
    runtime.record_input(item.mode, item.clipboard.echo_text_for_submit(text))?;
    // 2. 【终端队列】【单轮执行】主动提交清除旧回执，再由统一轮次入口处理流式事件
    let _ = agent.discard_stale_external_completion_notices().await;
    let submission = repl_runner_submission(
        chat_input,
        item.mode,
        reasoning_mode,
        tool_call_mode,
        stream_render_options(config),
        false,
    );
    execute_repl_turn(paths, config, agent, runtime, owner_key, submission)
        .await
        .map(Some)
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
    // 复用同一套判定：主循环单独分发的 /plan、/ps 等命令也要拦住，
    // 否则会从队列里被当成聊天正文发给模型
    super::super::repl_commands::is_stream_command_text(text)
}

/// 【终端队列】【失败提示】说明尚未执行的消息仍可在队列管理面板中查看或发送。
///
/// 参数:
/// - `runtime`: TUI 运行期
///
/// 返回:
/// - 记录是否成功
fn notify_preserved_queue(runtime: &mut ReplRuntime) -> Result<()> {
    runtime.pause_submission_queue();
    let count = runtime.queue_len();
    if count == 0 {
        return Ok(());
    }
    runtime.record_meta(if crate::i18n::is_zh() {
        format!("已保留 {count} 条未执行的排队消息，可按 Ctrl+↑ 管理")
    } else {
        format!("kept {count} queued messages that have not run; use Ctrl+Up to manage them")
    })
}

/// 【终端队列】【命令恢复】把控制命令连同附件交回空输入框，不覆盖现有草稿。
///
/// 参数:
/// - `runtime`: TUI 运行期
/// - `position`: 命令原队列位置
/// - `item`: 带原始附件的控制命令
///
/// 返回: 提示记录结果；已有草稿时命令继续留在队列中
fn restore_control_submission(
    runtime: &mut ReplRuntime,
    position: usize,
    item: QueuedSubmission,
) -> Result<()> {
    let message = if runtime.stream_draft().text.trim().is_empty() {
        *runtime.stream_draft_mut() = StreamComposerDraft {
            cursor: item.text.chars().count(),
            text: item.text,
            clipboard: item.clipboard,
            mode: Some(item.mode),
            ..StreamComposerDraft::default()
        };
        crate::i18n::text(
            "slash commands are not run from the queue; the text was returned to the input box",
            "斜杠命令不会从队列执行，已交还输入框",
        )
    } else {
        runtime.restore_queued_submission(position, item);
        crate::i18n::text(
            "the input box has a draft; the command remains in the queue",
            "输入框已有草稿，控制命令继续保留在队列中",
        )
    };
    runtime.record_meta(message.to_string())
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
#[path = "submission_queue_tests.rs"]
mod recovery_tests;

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
            crate::runner::RunnerSubmissionKind::UserInput(input) if input.image_urls.len() == 1
        ));
    }
}
