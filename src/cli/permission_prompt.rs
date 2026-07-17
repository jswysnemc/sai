use crate::permission::{
    PermissionDecision, PermissionInteractionState, PermissionRequest, PermissionTransition,
};
use crate::render::{
    clear_rendered_rows, render_permission_controls, render_permission_title, rendered_visual_rows,
};
use anyhow::Result;
use crossterm::cursor::{Hide, Show};
use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use crossterm::execute;
use crossterm::terminal;
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
        let _ = terminal::disable_raw_mode();
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
pub(super) fn read_permission_decision(request: &PermissionRequest) -> Result<PermissionDecision> {
    if io::stdin().is_terminal() && io::stdout().is_terminal() {
        read_terminal_decision(request)
    } else {
        read_line_decision(request)
    }
}

/// 在交互式终端 stdout 按固定行数擦除重绘权限菜单。
///
/// 参数:
/// - `request`: 当前权限请求
///
/// 返回:
/// - 用户提交的权限决定
fn read_terminal_decision(request: &PermissionRequest) -> Result<PermissionDecision> {
    let mut state = PermissionInteractionState::new();
    let mut stdout = io::stdout();
    // 1. 先刷掉上游流式输出，再进入 raw mode
    stdout.flush()?;
    terminal::enable_raw_mode()?;
    execute!(stdout, Hide)?;
    let _guard = TerminalGuard { show_cursor: true };
    // 2. 记录当前菜单占用视觉行数，上下键只擦这些行，避免标题叠加
    let mut painted_rows = paint_menu(&mut stdout, request, &state)?;
    loop {
        let event = event::read()?;
        // Ctrl+C / Ctrl+D 视为拒绝并退出
        if is_interrupt(&event) {
            erase_menu(&mut stdout, painted_rows)?;
            execute!(stdout, Show)?;
            stdout.flush()?;
            return Ok(PermissionDecision::Deny { reply: None });
        }
        match state.handle_event(event) {
            PermissionTransition::Continue => {
                erase_menu(&mut stdout, painted_rows)?;
                painted_rows = paint_menu(&mut stdout, request, &state)?;
            }
            PermissionTransition::Submit(decision) => {
                erase_menu(&mut stdout, painted_rows)?;
                execute!(stdout, Show)?;
                stdout.flush()?;
                return Ok(decision);
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
fn is_interrupt(event: &Event) -> bool {
    let Event::Key(key) = event else {
        return false;
    };
    if key.kind == KeyEventKind::Release {
        return false;
    }
    matches!(
        (key.code, key.modifiers.contains(KeyModifiers::CONTROL)),
        (KeyCode::Char('c') | KeyCode::Char('C') | KeyCode::Char('d') | KeyCode::Char('D'), true)
    )
}

/// 绘制完整权限菜单，返回占用的视觉行数。
///
/// 参数:
/// - `stdout`: 标准输出
/// - `request`: 权限请求
/// - `state`: 交互状态
///
/// 返回:
/// - 菜单占用的视觉行数
fn paint_menu(
    stdout: &mut io::Stdout,
    request: &PermissionRequest,
    state: &PermissionInteractionState,
) -> Result<usize> {
    let title = render_permission_title(&request.tool);
    let controls = render_permission_controls(state.selected(), state.reply_draft());
    // raw mode 下必须 \r\n，否则会阶梯缩进
    let logical = format!("{title}\n{controls}");
    let rows = rendered_visual_rows(&logical).max(1);
    let mut output = logical.replace('\n', "\r\n");
    output.push_str("\r\n");
    write!(stdout, "{output}")?;
    stdout.flush()?;
    // 末尾 \r\n 会把光标移到菜单下一空行起点；擦除时行数按逻辑内容行计
    Ok(rows)
}

/// 擦除刚画过的权限菜单。
///
/// 参数:
/// - `stdout`: 标准输出
/// - `painted_rows`: 上一次绘制的视觉行数
///
/// 返回:
/// - 擦除是否成功
fn erase_menu(stdout: &mut io::Stdout, painted_rows: usize) -> Result<()> {
    // 光标在菜单末尾下一行；先回上一行内容区，再按行上移并清行
    write!(stdout, "\r\x1b[2K")?;
    write!(stdout, "{}", clear_rendered_rows(painted_rows))?;
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
    println!("{}", render_permission_title(&request.tool));
    println!("1. 允许一次\n2. 拒绝\n3. 拒绝并告诉 Sai 如何调整");
    print!("选择 [1-3]，也可直接输入拒绝原因: ");
    io::stdout().flush()?;
    let mut answer = String::new();
    io::stdin().read_line(&mut answer)?;
    let answer = answer.trim();
    if answer == "1" || answer.eq_ignore_ascii_case("y") || answer.eq_ignore_ascii_case("yes") {
        return Ok(PermissionDecision::Allow);
    }
    if answer == "3" {
        print!("告诉 Sai 应如何调整: ");
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
}
