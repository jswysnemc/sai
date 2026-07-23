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
/// - 非空草稿文本
pub(super) fn take_stream_draft_prefill(runtime: &mut ReplRuntime) -> Option<String> {
    let text = runtime.stream_draft().text.trim().to_string();
    if text.is_empty() {
        return None;
    }
    let mode = runtime.stream_draft().mode;
    let draft = runtime.stream_draft_mut();
    *draft = Default::default();
    draft.mode = mode;
    Some(text)
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
        for item in queued {
            *mode = item.mode;
            let text = item.text.trim().to_string();
            if text.is_empty() {
                continue;
            }
            // 控制命令和 shell 在队列中仅作为用户消息处理
            input_history.push(text.clone());
            runtime.record_user(*mode, text.clone())?;
            if agent.mode() != *mode {
                let registry = build_repl_tool_registry(config, paths, *mode)?;
                agent.switch_mode(*mode, registry);
            }
            agent.prepare_for_turn()?;
            let chat_input = crate::clipboard::ClipboardChatInput {
                message: text.clone(),
                image_url: None,
            };
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
            if outcome.interrupted {
                restore_leftover_draft(runtime, outcome.leftover_draft);
                return Ok(());
            }
            if let Err(error) = outcome.result {
                runtime.record_meta(error.to_string())?;
                restore_leftover_draft(runtime, outcome.leftover_draft);
                return Ok(());
            }
            // 队列下一项会清空草稿，因此先把残留文本插回队首
            if let Some(draft) = outcome.leftover_draft {
                let text = draft.trim().to_string();
                if !text.is_empty() {
                    let queued_mode = runtime.stream_mode(*mode);
                    runtime.prepend_submission(queued_mode, text);
                }
            }
            // 本轮执行中新入队的项目由外层循环继续处理
        }
    }
    Ok(())
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
