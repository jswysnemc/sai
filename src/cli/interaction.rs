use super::repl_runtime::ReplRuntime;
use super::{permission_prompt, terminal_restore};
use crate::agent::AgentEvent;
use crate::render;
use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
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
        AgentEvent::Chunk(chunk) => renderer.write_chunk(chunk),
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
        AgentEvent::ToolProgress { name, message } => renderer.write_tool_progress(&name, &message),
        AgentEvent::ToolProgressIdentified { name, message, .. } => {
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
        AgentEvent::EngineReady { engine, version } => {
            renderer.prepare_for_external_output()?;
            println!(
                "\x1b[2m• {} {engine} {version}\x1b[0m",
                crate::render::terminal_text("connected to", "已连接")
            );
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

/// 在 TUI 原始模式中读取权限选择，并更新既有工具视图。
///
/// 参数:
/// - `request`: 已经写入 transcript 的权限请求
/// - `runtime`: REPL 运行期
///
/// 返回:
/// - 权限决定提交结果
pub(super) fn prompt_permission_request_tui(
    request: &crate::permission::PermissionRequest,
    runtime: &std::cell::RefCell<&mut ReplRuntime>,
) -> Result<()> {
    use crate::permission::{PermissionInteractionState, PermissionTransition};

    let mut state = PermissionInteractionState::new();
    let mut stdout = io::stdout();
    // 1. 独占 raw 输入，避免与主循环输入框事件竞争
    let mut terminal_guard = terminal_restore::TerminalInputGuard::enable(&mut stdout, true)?;
    // 2. 选择项附着在工具视图下方（working 动效已在 sink 入口暂停）
    {
        let mut rt = runtime.borrow_mut();
        rt.update_permission_choice(&request.id, state.selected())?;
        rt.update_permission_reply(&request.id, state.reply_draft().map(str::to_string))?;
    }

    let result = (|| -> Result<()> {
        loop {
            // 自动审核已提交时退出交互，交还主循环
            if !crate::permission::is_permission_pending(&request.id) {
                return Ok(());
            }
            if !event::poll(std::time::Duration::from_millis(120))? {
                continue;
            }
            let event = event::read()?;
            // Ctrl+C / Ctrl+D 视为拒绝，避免审计循环无法退出
            if permission_prompt::is_interrupt(&event) {
                if !crate::permission::is_permission_pending(&request.id) {
                    return Ok(());
                }
                return crate::permission::decide_permission(
                    &request.id,
                    crate::permission::PermissionDecision::Deny { reply: None },
                );
            }
            if let Event::Resize(cols, rows) = event {
                let mut rt = runtime.borrow_mut();
                rt.observe_input_resize(cols, rows);
                rt.update_permission_choice(&request.id, state.selected())?;
                rt.update_permission_reply(&request.id, state.reply_draft().map(str::to_string))?;
                continue;
            }
            // Shift+Tab / BackTab：立即切到 YOLO 并放行当前会话全部待审
            if let Event::Key(key) = &event {
                if key.kind != KeyEventKind::Release {
                    let shift_tab = matches!(key.code, KeyCode::BackTab)
                        || (matches!(key.code, KeyCode::Tab)
                            && key.modifiers.contains(KeyModifiers::SHIFT));
                    if shift_tab {
                        {
                            let mut rt = runtime.borrow_mut();
                            rt.stream_draft_mut().mode = Some(crate::agent::AgentMode::Yolo);
                            let _ = rt.apply_stream_mode_live(crate::agent::AgentMode::Yolo);
                        }
                        let _ =
                            crate::permission::allow_all_pending_for_session(&request.session_id);
                        return Ok(());
                    }
                }
            }
            match state.handle_event(event) {
                PermissionTransition::Continue => {
                    let mut rt = runtime.borrow_mut();
                    rt.update_permission_choice(&request.id, state.selected())?;
                    rt.update_permission_reply(
                        &request.id,
                        state.reply_draft().map(str::to_string),
                    )?;
                }
                PermissionTransition::Submit(decision) => {
                    if !crate::permission::is_permission_pending(&request.id) {
                        return Ok(());
                    }
                    return crate::permission::decide_permission(&request.id, decision);
                }
            }
        }
    })();

    // 3. 恢复终端模式，交回后续流式输出和下一轮输入
    let _ = terminal_guard.finish(&mut stdout);
    result
}

/// 在终端读取结构化提问答案。
///
/// 参数:
/// - `pending`: 待回答提问
///
/// 返回:
/// - 是否成功提交回答
fn prompt_question_request(pending: &crate::question::PendingQuestion) -> Result<()> {
    let response = crate::question_tui::ask(&pending.request)
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
    runtime: &std::cell::RefCell<&mut ReplRuntime>,
) -> Result<()> {
    let mut stdout = io::stdout();
    // 1. 独占 raw 输入，避免与主循环输入框事件竞争
    let mut terminal_guard = terminal_restore::TerminalInputGuard::enable(&mut stdout, true)?;
    {
        let mut rt = runtime.borrow_mut();
        rt.pause_for_permission_prompt()?;
    }

    let response = crate::question_tui::ask(&pending.request)
        .unwrap_or_else(|err| crate::question::QuestionResponse::Unavailable(err.to_string()));

    // 2. 恢复终端模式；提问面板直接写过终端，受管区域需要在下次同步前重启
    let _ = terminal_guard.finish(&mut stdout);
    runtime.borrow_mut().mark_desynced();
    crate::question::resolve_question(&pending.id, response)
}
