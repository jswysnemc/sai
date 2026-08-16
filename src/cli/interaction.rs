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

/// 在 TUI 原始模式中安全征询 SSH 秘密或确认。
///
/// 口令/密码输入全程不回显；主机指纹与高危命令走是/否确认。用户应答经独立安全通道
/// 直达后端工具，绝不写入 transcript 或模型上下文。
///
/// 参数:
/// - `request`: 待处理的交互征询（不含秘密）
/// - `runtime`: REPL 运行期
///
/// 返回:
/// - 应答提交结果
pub(super) fn prompt_ssh_secret_request_tui(
    request: &crate::ssh::SecretRequest,
    runtime: &std::cell::RefCell<&mut ReplRuntime>,
) -> Result<()> {
    let mut stdout = io::stdout();
    // 1. 独占 raw 输入，避免与主循环输入框事件竞争
    let mut terminal_guard = terminal_restore::TerminalInputGuard::enable(&mut stdout, true)?;
    {
        let mut rt = runtime.borrow_mut();
        rt.pause_for_permission_prompt()?;
    }
    let response = read_ssh_secret_response(&mut stdout, request)
        .unwrap_or(crate::ssh::SecretResponse::Cancelled);
    // 2. 恢复终端模式；安全输入直接写过终端，受管区域需在下次同步前重启
    let _ = terminal_guard.finish(&mut stdout);
    runtime.borrow_mut().mark_desynced();
    // 仅在仍等待时提交，避免与后端超时竞态
    if crate::ssh::is_pending(&request.id) {
        let _ = crate::ssh::submit_secret(&request.id, response);
    }
    Ok(())
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
        prompt_ssh_secret_request_cli(&request)?;
    }
    Ok(())
}

/// 在 CLI 流式输出中安全征询 SSH 秘密或确认。
///
/// 非交互终端无法安全输入，直接取消并交由后端给出明确提示。
///
/// 参数:
/// - `request`: 待处理的交互征询（不含秘密）
///
/// 返回:
/// - 应答提交结果
pub(super) fn prompt_ssh_secret_request_cli(request: &crate::ssh::SecretRequest) -> Result<()> {
    use std::io::IsTerminal;
    let mut stdout = io::stdout();
    if !(io::stdin().is_terminal() && stdout.is_terminal()) {
        if crate::ssh::is_pending(&request.id) {
            let _ = crate::ssh::submit_secret(&request.id, crate::ssh::SecretResponse::Cancelled);
        }
        return Ok(());
    }
    stdout.flush()?;
    crossterm::terminal::enable_raw_mode()?;
    let response = read_ssh_secret_response(&mut stdout, request)
        .unwrap_or(crate::ssh::SecretResponse::Cancelled);
    let _ = crossterm::terminal::disable_raw_mode();
    println!();
    if crate::ssh::is_pending(&request.id) {
        let _ = crate::ssh::submit_secret(&request.id, response);
    }
    Ok(())
}

/// 绘制征询提示并读取用户应答（要求调用方已进入原始模式）。
///
/// 参数:
/// - `stdout`: 标准输出
/// - `request`: 交互征询
///
/// 返回:
/// - 用户应答
fn read_ssh_secret_response(
    stdout: &mut io::Stdout,
    request: &crate::ssh::SecretRequest,
) -> Result<crate::ssh::SecretResponse> {
    use crate::ssh::{InteractiveKind, SecretResponse};
    // raw 模式下必须使用 \r\n，否则阶梯缩进
    write!(
        stdout,
        "\r\n\x1b[38;5;39m● SSH · {}\x1b[0m\r\n",
        request.host_label
    )?;
    for line in request.prompt.split('\n') {
        write!(stdout, "{line}\r\n")?;
    }
    if let Some(fingerprint) = &request.fingerprint {
        write!(stdout, "SHA256 {fingerprint}\r\n")?;
        if request.changed {
            write!(
                stdout,
                "\x1b[33m警告：该主机指纹与 known_hosts 记录不一致\x1b[0m\r\n"
            )?;
        }
    }
    match request.kind {
        InteractiveKind::Passphrase | InteractiveKind::Password => {
            write!(stdout, "输入后回车提交，Esc 取消（不回显）: ")?;
            stdout.flush()?;
            Ok(match read_hidden_line(&request.id)? {
                Some(secret) => SecretResponse::Provided(secret),
                None => SecretResponse::Cancelled,
            })
        }
        InteractiveKind::HostKey | InteractiveKind::DangerCommand => {
            write!(stdout, "确认继续？[y/N]: ")?;
            stdout.flush()?;
            Ok(match read_confirmation(&request.id)? {
                Some(confirmed) => SecretResponse::Confirmed(confirmed),
                None => SecretResponse::Cancelled,
            })
        }
    }
}

/// 在原始模式下不回显地读取一行秘密。
///
/// 参数:
/// - `request_id`: 关联请求标识，用于在后端超时后自动退出
///
/// 返回:
/// - 提交的秘密；取消或中断时为 `None`
fn read_hidden_line(request_id: &str) -> Result<Option<String>> {
    let mut buffer = String::new();
    loop {
        if !event::poll(std::time::Duration::from_millis(150))? {
            // 后端超时撤销请求后不再阻塞等待按键
            if !crate::ssh::is_pending(request_id) {
                return Ok(None);
            }
            continue;
        }
        let event = event::read()?;
        if permission_prompt::is_interrupt(&event) {
            return Ok(None);
        }
        let Event::Key(key) = event else { continue };
        if key.kind == KeyEventKind::Release {
            continue;
        }
        match key.code {
            KeyCode::Enter => return Ok(Some(buffer)),
            KeyCode::Esc => return Ok(None),
            KeyCode::Backspace => {
                buffer.pop();
            }
            KeyCode::Char(value) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                buffer.push(value);
            }
            _ => {}
        }
    }
}

/// 在原始模式下读取是/否确认。
///
/// 参数:
/// - `request_id`: 关联请求标识，用于在后端超时后自动退出
///
/// 返回:
/// - 用户是否确认；取消或中断时为 `None`
fn read_confirmation(request_id: &str) -> Result<Option<bool>> {
    loop {
        if !event::poll(std::time::Duration::from_millis(150))? {
            if !crate::ssh::is_pending(request_id) {
                return Ok(None);
            }
            continue;
        }
        let event = event::read()?;
        if permission_prompt::is_interrupt(&event) {
            return Ok(None);
        }
        let Event::Key(key) = event else { continue };
        if key.kind == KeyEventKind::Release {
            continue;
        }
        match key.code {
            KeyCode::Char('y') | KeyCode::Char('Y') => return Ok(Some(true)),
            KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc | KeyCode::Enter => {
                return Ok(Some(false))
            }
            _ => {}
        }
    }
}
