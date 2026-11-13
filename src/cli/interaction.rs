use super::repl_runtime::ReplRuntime;
use super::{permission_prompt, terminal_restore};
use crate::agent::AgentEvent;
use crate::render;
use anyhow::Result;
use std::io::{self, Write};

/// 将单次 CLI Agent 事件转发到流式渲染器或权限交互。
///
/// 参数:
/// - `renderer`: CLI 流式渲染器
/// - `event`: Agent 事件
///
/// 返回:
/// - 事件处理结果
pub(super) fn handle_agent_event(
    renderer: &mut render::StreamRenderer,
    event: AgentEvent,
) -> Result<()> {
    match event {
        AgentEvent::ResponseReset {
            content_chars,
            reasoning_chars,
            ..
        } => {
            if content_chars + reasoning_chars > 0 {
                renderer.prepare_for_external_output()?;
                println!(
                    "{}",
                    crate::i18n::text(
                        "Response interrupted; regenerating the incomplete response.",
                        "响应中断，正在重新生成未完成的回复。"
                    )
                );
            }
            Ok(())
        }
        AgentEvent::Chunk(chunk) => renderer.write_chunk(chunk),
        AgentEvent::InterMessage(message) => {
            renderer.prepare_for_external_output()?;
            println!("\x1b[38;5;39m●\x1b[0m {}", message.content);
            Ok(())
        }
        AgentEvent::WaitingExternal => Ok(()),
        // CLI：重连只刷新底部状态行动效，不打印历史行（对齐 Codex StreamError）
        AgentEvent::Reconnecting {
            attempt,
            max_attempts,
        } => renderer.note_reconnecting(attempt, max_attempts),
        AgentEvent::ContextUpdated(_) => Ok(()),
        AgentEvent::ToolCall { name, arguments } => renderer.write_tool_call(&name, &arguments),
        AgentEvent::ToolCallIdentified {
            name, arguments, ..
        } => renderer.write_tool_call(&name, &arguments),
        AgentEvent::ToolCallProgress(progress) => renderer.write_tool_call_progress(&progress),
        AgentEvent::ToolResult { name, ok, output } => {
            renderer.write_tool_result(&name, ok, &output)
        }
        AgentEvent::ToolResultIdentified {
            name, ok, output, ..
        } => renderer.write_tool_result(&name, ok, &output),
        AgentEvent::ToolProgress { name, message } => {
            if crate::ssh::is_secret_marker(&message) {
                return handle_ssh_secret_marker_cli(renderer, &message);
            }
            renderer.write_tool_progress(&name, &message)
        }
        AgentEvent::ToolProgressIdentified { name, message, .. } => {
            if crate::ssh::is_secret_marker(&message) {
                return handle_ssh_secret_marker_cli(renderer, &message);
            }
            renderer.write_tool_progress(&name, &message)
        }
        AgentEvent::PermissionRequested(request) => {
            // 停掉末行动效与 live 行，再在 stdout 画可导航审计菜单
            renderer.prepare_for_external_output()?;
            io::stdout().flush()?;
            match prompt_permission_request(&request)? {
                Some(decision) => {
                    // 拒绝决定已单独展示，抑制随后同名工具的失败输出块避免重复
                    if matches!(decision, crate::permission::PermissionDecision::Deny { .. }) {
                        renderer.suppress_denied_result(&request.tool);
                    }
                }
                None => {
                    // 自动审核已决出：结果在 PermissionResolved 中展示
                }
            }
            Ok(())
        }
        AgentEvent::PermissionResolved { decision, .. } => {
            // 人工与自动审核统一在此打印结果（prompt 不再打印，避免竞态重复）
            println!(
                "{}",
                crate::render::render_permission_decision_for(
                    &decision,
                    crate::render::PermissionView::Cli
                )
            );
            Ok(())
        }
        AgentEvent::QuestionRequested(pending) => {
            renderer.prepare_for_external_output()?;
            io::stdout().flush()?;
            prompt_question_request(&pending)
        }
        AgentEvent::QuestionResolved { .. } => Ok(()),
        AgentEvent::CompactionStarted { turn_count, model } => {
            renderer.write_compaction_started(turn_count, &model)
        }
        AgentEvent::CompactionDelta { text } => renderer.write_compaction_delta(text),
        AgentEvent::CompactionFinished { applied, error, .. } => {
            renderer.write_compaction_finished(applied, error.as_ref())
        }
        AgentEvent::JevPreselect { phase, detail } => {
            if let Some(line) = crate::render::format_jev_preselect(phase.as_str(), detail.as_str())
            {
                renderer.prepare_for_external_output()?;
                println!("\x1b[2m{line}\x1b[0m");
            }
            Ok(())
        }
        AgentEvent::FlushContent => renderer.flush_content(),
        AgentEvent::ExternalOutput => renderer.prepare_for_external_output(),
    }
}

/// 在终端读取权限允许、拒绝或拒绝原因。
///
/// 参数:
/// - `request`: 待处理权限请求
///
/// 返回:
/// - 已提交给权限 Broker 的用户决定
pub(super) fn prompt_permission_request(
    request: &crate::permission::PermissionRequest,
) -> Result<Option<crate::permission::PermissionDecision>> {
    // 1. 读取人工选择；None 表示自动审核已先决出
    let Some(decision) = permission_prompt::read_permission_decision(request)? else {
        return Ok(None);
    };
    // 2. 仅在请求仍 pending 时提交，避免与自动审核竞态
    if crate::permission::is_permission_pending(&request.id) {
        crate::permission::decide_permission(&request.id, decision.clone())?;
        Ok(Some(decision))
    } else {
        Ok(None)
    }
}

/// 在终端读取结构化提问答案。
///
/// 参数:
/// - `pending`: 待回答提问
///
/// 返回:
/// - 是否成功提交回答
fn prompt_question_request(pending: &crate::question::PendingQuestion) -> Result<()> {
    let response = super::plan_review::ask(pending, false, None)
        .unwrap_or_else(|err| crate::question::QuestionResponse::Unavailable(err.to_string()));
    crate::question::resolve_question(&pending.id, response)
}

/// 在 TUI 原始模式中读取结构化提问答案。
///
/// 参数:
/// - `pending`: 待回答提问
/// - `runtime`: REPL 运行期
///
/// 返回:
/// - 是否成功提交回答
pub(super) fn prompt_question_request_tui(
    pending: &crate::question::PendingQuestion,
    runtime: &mut ReplRuntime,
) -> Result<()> {
    let mut stdout = io::stdout();
    // 1. 独占 raw 输入，避免与主循环输入框事件竞争；全屏会话保持不变
    let mut terminal_guard = terminal_restore::TerminalInputGuard::enable(&mut stdout, true)?;
    runtime.prepare_question_prompt()?;
    let placement = runtime.question_overlay_placement();

    let response = super::plan_review::ask(pending, true, Some(placement))
        .unwrap_or_else(|err| crate::question::QuestionResponse::Unavailable(err.to_string()));

    // 2. 【终端提问】【底部覆盖】清掉提问覆盖层后由 runtime 恢复输入框与历史
    let _ = terminal_guard.finish(&mut stdout);
    runtime.finish_question_prompt()?;
    crate::question::resolve_question(&pending.id, response)
}

/// 处理 CLI 流式输出中的 SSH 秘密交互带外标记。
///
/// 请求标记触发安全输入；结束标记在 CLI 无需额外动作。
///
/// 参数:
/// - `renderer`: CLI 流式渲染器
/// - `message`: 工具进度中的带外标记
///
/// 返回:
/// - 处理结果
fn handle_ssh_secret_marker_cli(
    renderer: &mut render::StreamRenderer,
    message: &str,
) -> Result<()> {
    if let Some(request) = crate::ssh::decode_progress_marker(message) {
        renderer.prepare_for_external_output()?;
        io::stdout().flush()?;
        super::ssh_prompt::prompt_ssh_secret_request_cli(&request)?;
    }
    Ok(())
}
