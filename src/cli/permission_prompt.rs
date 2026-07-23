use crate::i18n::text as t;
use crate::permission::{
    PermissionDecision, PermissionInteractionState, PermissionRequest, PermissionTransition,
};
use crate::render::{render_permission_controls, render_permission_title, rendered_visual_rows};
use anyhow::Result;
use crossterm::cursor::{Hide, MoveTo, Show};
use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use crossterm::terminal::{Clear, ClearType};
use crossterm::{execute, queue};
use std::io::{self, IsTerminal, Write};

/// 终端原始模式与光标恢复守卫。
struct TerminalGuard {
    show_cursor: bool,
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let mut stdout = io::stdout();
        if self.show_cursor {
            let _ = execute!(stdout, Show);
        }
        let _ = crossterm::terminal::disable_raw_mode();
        let _ = stdout.flush();
    }
}

/// 读取单次 CLI 权限决定。
///
/// 参数:
/// - `request`: 当前权限请求，用于展示工具名
///
/// 返回:
/// - 用户选择的允许或拒绝决定
pub(super) fn read_permission_decision(
    request: &PermissionRequest,
) -> Result<Option<PermissionDecision>> {
    if io::stdin().is_terminal() && io::stdout().is_terminal() {
        read_terminal_decision(request)
    } else {
        // 管道输入无法与自动审核并行轮询，仅人工决定
        Ok(Some(read_line_decision(request)?))
    }
}

/// 在交互式终端 stdout 用固定锚点擦除重绘权限菜单。
///
/// 菜单始终从记录的锚点行开始绘制，重绘前从锚点清到屏幕底部，
/// 不再按估算行数上移擦除，避免测量偏差留下菜单残影。
///
/// 参数:
/// - `request`: 当前权限请求
///
/// 返回:
/// - 用户提交的权限决定
fn read_terminal_decision(request: &PermissionRequest) -> Result<Option<PermissionDecision>> {
    let mut state = PermissionInteractionState::new();
    let mut stdout = io::stdout();
    // 1. 先刷掉上游流式输出，再进入 raw mode
    stdout.flush()?;
    crossterm::terminal::enable_raw_mode()?;
    execute!(stdout, Hide)?;
    let _guard = TerminalGuard { show_cursor: true };
    // 2. 预留菜单空间并记录锚点行，之后的重绘都从锚点开始
    let mut anchor = reserve_menu_anchor(&mut stdout, menu_rows(request, &state))?;
    // 初始绘制含自动审核状态行
    paint_menu_at(&mut stdout, anchor, request, &state, request.auto_audit)?;
    loop {
        // 自动审核已提交决定时收起菜单并退出，不再阻塞
        if !crate::permission::is_permission_pending(&request.id) {
            erase_menu_at(&mut stdout, anchor)?;
            execute!(stdout, Show)?;
            stdout.flush()?;
            return Ok(None);
        }
        if !event::poll(std::time::Duration::from_millis(120))? {
            // 刷新自动审核状态提示
            paint_menu_at(&mut stdout, anchor, request, &state, request.auto_audit)?;
            continue;
        }
        let event = event::read()?;
        // Ctrl+C / Ctrl+D 视为拒绝并退出
        if is_interrupt(&event) {
            erase_menu_at(&mut stdout, anchor)?;
            execute!(stdout, Show)?;
            stdout.flush()?;
            return Ok(Some(PermissionDecision::Deny { reply: None }));
        }
        if let Event::Resize(_, _) = event {
            anchor = reserve_menu_anchor(&mut stdout, menu_rows(request, &state))?;
            paint_menu_at(&mut stdout, anchor, request, &state, request.auto_audit)?;
            continue;
        }
        // Shift+Tab / BackTab：视为切换到 YOLO，立即放行待审
        if let Event::Key(key) = &event {
            if key.kind != KeyEventKind::Release {
                let shift_tab = matches!(key.code, KeyCode::BackTab)
                    || (matches!(key.code, KeyCode::Tab)
                        && key.modifiers.contains(KeyModifiers::SHIFT));
                if shift_tab {
                    let _ = crate::permission::allow_all_pending_for_session(&request.session_id);
                    erase_menu_at(&mut stdout, anchor)?;
                    execute!(stdout, Show)?;
                    stdout.flush()?;
                    return Ok(None);
                }
            }
        }
        match state.handle_event(event) {
            PermissionTransition::Continue => {
                // 回复草稿可能换行增高，重绘前确认锚点下空间仍然充足
                anchor = ensure_anchor_space(&mut stdout, anchor, menu_rows(request, &state))?;
                paint_menu_at(&mut stdout, anchor, request, &state, request.auto_audit)?;
            }
            PermissionTransition::Submit(decision) => {
                erase_menu_at(&mut stdout, anchor)?;
                execute!(stdout, Show)?;
                stdout.flush()?;
                return Ok(Some(decision));
            }
        }
    }
}

/// 判断是否为中断快捷键。
///
/// 参数:
/// - `event`: 终端事件
///
/// 返回:
/// - 是否应中断权限交互
pub(super) fn is_interrupt(event: &Event) -> bool {
    let Event::Key(key) = event else {
        return false;
    };
    if key.kind == KeyEventKind::Release {
        return false;
    }
    matches!(
        (key.code, key.modifiers.contains(KeyModifiers::CONTROL)),
        (
            KeyCode::Char('c') | KeyCode::Char('C') | KeyCode::Char('d') | KeyCode::Char('D'),
            true
        )
    )
}

/// 计算当前菜单占用的视觉行数。
///
/// 参数:
/// - `request`: 权限请求
/// - `state`: 交互状态
///
/// 返回:
/// - 菜单视觉行数
fn menu_rows(request: &PermissionRequest, state: &PermissionInteractionState) -> u16 {
    let status = crate::render::render_auto_audit_status(request.auto_audit);
    let logical = if status.is_empty() {
        format!(
            "{}\n{}",
            render_permission_title(&request.tool, Some(&request.arguments)),
            render_permission_controls(state.selected(), state.reply_draft())
        )
    } else {
        format!(
            "{}\n{}\n{}",
            render_permission_title(&request.tool, Some(&request.arguments)),
            status,
            render_permission_controls(state.selected(), state.reply_draft())
        )
    };
    rendered_visual_rows(&logical)
        .max(1)
        .min(usize::from(u16::MAX)) as u16
}

/// 预留菜单空间并返回锚点行。
///
/// 光标位于行中时先换行；屏幕剩余行数不足时用换行滚动腾出空间，
/// 被顶上去的内容自然进入终端 scrollback。
///
/// 参数:
/// - `stdout`: 标准输出
/// - `rows`: 需要的行数
///
/// 返回:
/// - 菜单锚点行（从零开始）
fn reserve_menu_anchor(stdout: &mut io::Stdout, rows: u16) -> Result<u16> {
    let (col, row) = crossterm::cursor::position().unwrap_or((0, 0));
    if col != 0 {
        write!(stdout, "\r\n")?;
        stdout.flush()?;
    }
    let (_, current) = crossterm::cursor::position().unwrap_or((0, row));
    ensure_anchor_space(stdout, current, rows)
}

/// 确认锚点之下有足够行数，不足时滚动终端并上移锚点。
///
/// 参数:
/// - `stdout`: 标准输出
/// - `anchor`: 当前锚点行
/// - `rows`: 菜单需要的行数
///
/// 返回:
/// - 调整后的锚点行
fn ensure_anchor_space(stdout: &mut io::Stdout, anchor: u16, rows: u16) -> Result<u16> {
    let (_, screen_rows) = crossterm::terminal::size().unwrap_or((80, 24));
    let screen_rows = screen_rows.max(1);
    let anchor = anchor.min(screen_rows.saturating_sub(1));
    let needed = rows.min(screen_rows);
    let overflow = (anchor.saturating_add(needed)).saturating_sub(screen_rows);
    if overflow == 0 {
        return Ok(anchor);
    }
    // 在屏幕底行输出换行触发真实滚动，菜单上方内容进入 scrollback
    queue!(stdout, MoveTo(0, screen_rows.saturating_sub(1)))?;
    for _ in 0..overflow {
        queue!(stdout, crossterm::style::Print("\r\n"))?;
    }
    stdout.flush()?;
    Ok(anchor.saturating_sub(overflow))
}

/// 从锚点行绘制完整权限菜单。
///
/// 参数:
/// - `stdout`: 标准输出
/// - `anchor`: 菜单锚点行
/// - `request`: 权限请求
/// - `state`: 交互状态
///
/// 返回:
/// - 绘制是否成功
fn paint_menu_at(
    stdout: &mut io::Stdout,
    anchor: u16,
    request: &PermissionRequest,
    state: &PermissionInteractionState,
    show_auto_audit: bool,
) -> Result<()> {
    let title = render_permission_title(&request.tool, Some(&request.arguments));
    let controls = render_permission_controls(state.selected(), state.reply_draft());
    // raw mode 下必须 \r\n，否则会阶梯缩进
    let body = if show_auto_audit {
        format!(
            "{title}\n{}\n{controls}",
            crate::render::render_auto_audit_status(true)
        )
    } else {
        format!("{title}\n{controls}")
    }
    .replace('\n', "\r\n");
    queue!(stdout, MoveTo(0, anchor), Clear(ClearType::FromCursorDown))?;
    write!(stdout, "{body}")?;
    stdout.flush()?;
    Ok(())
}

/// 擦除锚点行之后的菜单区域。
///
/// 参数:
/// - `stdout`: 标准输出
/// - `anchor`: 菜单锚点行
///
/// 返回:
/// - 擦除是否成功
fn erase_menu_at(stdout: &mut io::Stdout, anchor: u16) -> Result<()> {
    queue!(stdout, MoveTo(0, anchor), Clear(ClearType::FromCursorDown))?;
    stdout.flush()?;
    Ok(())
}

/// 在管道或重定向输入中读取编号和拒绝原因。
///
/// 参数:
/// - `request`: 权限请求
///
/// 返回:
/// - 用户提交的权限决定
fn read_line_decision(request: &PermissionRequest) -> Result<PermissionDecision> {
    println!(
        "{}",
        render_permission_title(&request.tool, Some(&request.arguments))
    );
    println!(
        "1. {}\n2. {}\n3. {}",
        t("Allow once", "允许一次"),
        t("Deny", "拒绝"),
        t("Deny and tell Sai how to adjust", "拒绝并告诉 Sai 如何调整")
    );
    print!(
        "{}: ",
        t(
            "Choose [1-3], or enter a denial reason directly",
            "选择 [1-3]，也可直接输入拒绝原因"
        )
    );
    io::stdout().flush()?;
    let mut answer = String::new();
    io::stdin().read_line(&mut answer)?;
    let answer = answer.trim();
    if answer == "1" || answer.eq_ignore_ascii_case("y") || answer.eq_ignore_ascii_case("yes") {
        return Ok(PermissionDecision::allow_once());
    }
    if answer == "3" {
        print!("{}: ", t("Tell Sai how to adjust", "告诉 Sai 应如何调整"));
        io::stdout().flush()?;
        let mut reply = String::new();
        io::stdin().read_line(&mut reply)?;
        return Ok(PermissionDecision::Deny {
            reply: (!reply.trim().is_empty()).then(|| reply.trim().to_string()),
        });
    }
    Ok(PermissionDecision::Deny {
        reply: (!answer.is_empty()
            && answer != "2"
            && !answer.eq_ignore_ascii_case("n")
            && !answer.eq_ignore_ascii_case("no"))
        .then(|| answer.to_string()),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::permission::PermissionTransition;
    use crate::render::PermissionChoice;
    use crossterm::event::{KeyEvent, KeyEventState};

    fn key(code: KeyCode) -> Event {
        Event::Key(KeyEvent {
            code,
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Press,
            state: KeyEventState::NONE,
        })
    }

    #[test]
    fn shared_state_machine_drives_cli_prompt_choices() {
        let mut state = PermissionInteractionState::new();
        assert_eq!(state.selected(), PermissionChoice::Allow);
        assert_eq!(
            state.handle_event(key(KeyCode::Down)),
            PermissionTransition::Continue
        );
        assert_eq!(state.selected(), PermissionChoice::Deny);
        assert_eq!(
            state.handle_event(key(KeyCode::Enter)),
            PermissionTransition::Submit(PermissionDecision::Deny { reply: None })
        );
    }

    #[test]
    fn ctrl_c_is_detected_as_interrupt() {
        let event = Event::Key(KeyEvent {
            code: KeyCode::Char('c'),
            modifiers: KeyModifiers::CONTROL,
            kind: KeyEventKind::Press,
            state: KeyEventState::NONE,
        });
        assert!(is_interrupt(&event));
    }

    #[test]
    fn menu_rows_count_title_and_controls() {
        let request = PermissionRequest {
            id: "id".to_string(),
            session_id: "session".to_string(),
            tool: "run_command".to_string(),
            arguments: r#"{"command":"date"}"#.to_string(),
            auto_audit: false,
        };
        let state = PermissionInteractionState::new();
        // 标题 + 三个选项 + 提示行
        assert!(menu_rows(&request, &state) >= 5);
    }
}
