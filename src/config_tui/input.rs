use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseEvent};
use std::time::Duration;

/// 【配置界面】【输入事件】统一承载键盘、鼠标与终端尺寸变更事件。
#[derive(Clone, Copy, Debug)]
pub(crate) enum ConfigEvent {
    Key(KeyEvent),
    Mouse(MouseEvent),
    Resize,
}

/// Ctrl+C / Ctrl+D 中断。
///
/// raw mode 下终端不会把 Ctrl+C 转成 SIGINT，它只是一个普通按键事件。
/// 不特殊处理的话配置界面里 Ctrl+C 完全没有反应，用户只能靠知道
/// 要按 q / Esc 才能出去。
#[derive(Debug)]
pub(crate) struct Interrupted;

impl std::fmt::Display for Interrupted {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("interrupted")
    }
}

impl std::error::Error for Interrupted {}

/// 判断是否为中断组合键。
///
/// 与 REPL 权限弹窗保持同一套约定（Ctrl+C / Ctrl+D）。
///
/// 参数:
/// - `event`: 终端键盘事件
///
/// 返回:
/// - 是否为中断组合键
fn is_interrupt(event: KeyEvent) -> bool {
    matches!(
        (event.code, event.modifiers.contains(KeyModifiers::CONTROL)),
        (
            KeyCode::Char('c') | KeyCode::Char('C') | KeyCode::Char('d') | KeyCode::Char('D'),
            true
        )
    )
}

/// 【配置界面】【事件读取】阻塞读取键盘、鼠标或尺寸调整事件。
///
/// 参数:
/// - 无
///
/// 返回:
/// - 归一化的配置界面输入事件
pub(crate) fn read_event() -> Result<ConfigEvent> {
    read_event_with_timeout(None).map(|event| event.expect("blocking read should return an event"))
}

/// 【配置界面】【超时事件】读取键盘、鼠标或尺寸调整事件，可选等待超时。
///
/// 参数:
/// - `timeout`: 等待终端事件的时长；为空时持续阻塞等待
///
/// 返回:
/// - 归一化输入事件；超时无事件时返回空
pub(crate) fn read_event_with_timeout(timeout: Option<Duration>) -> Result<Option<ConfigEvent>> {
    loop {
        // 1. 按需等待终端输入，超时后返回空值
        if let Some(timeout) = timeout {
            if !event::poll(timeout)? {
                return Ok(None);
            }
        }
        // 2. 分发键盘、鼠标与尺寸调整；按键先过滤释放态并响应中断键
        match event::read()? {
            Event::Key(event) => {
                if !is_actionable_key_event(event) {
                    continue;
                }
                if is_interrupt(event) {
                    return Err(Interrupted.into());
                }
                return Ok(Some(ConfigEvent::Key(event)));
            }
            Event::Mouse(mouse) => return Ok(Some(ConfigEvent::Mouse(mouse))),
            Event::Resize(_, _) => return Ok(Some(ConfigEvent::Resize)),
            _ => {}
        }
    }
}

/// 判断键盘事件是否可以驱动配置界面操作。
///
/// 参数:
/// - `event`: 终端键盘事件
///
/// 返回:
/// - 是否属于按下或重复输入事件
fn is_actionable_key_event(event: KeyEvent) -> bool {
    event.kind != KeyEventKind::Release
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{KeyEventKind, KeyEventState, KeyModifiers};

    /// 验证配置界面忽略 Windows 控制台残留的按键释放事件。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 无
    #[test]
    fn ignores_key_release_events() {
        let release = KeyEvent {
            code: KeyCode::Enter,
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Release,
            state: KeyEventState::NONE,
        };

        assert!(!is_actionable_key_event(release));
    }

    /// Ctrl+C / Ctrl+D 被识别为中断。
    #[test]
    fn detects_interrupt_keys() {
        let key = |code, modifiers| KeyEvent {
            code,
            modifiers,
            kind: KeyEventKind::Press,
            state: KeyEventState::NONE,
        };
        assert!(is_interrupt(key(KeyCode::Char('c'), KeyModifiers::CONTROL)));
        assert!(is_interrupt(key(KeyCode::Char('d'), KeyModifiers::CONTROL)));
        assert!(!is_interrupt(key(KeyCode::Char('c'), KeyModifiers::NONE)));
        assert!(!is_interrupt(key(
            KeyCode::Char('a'),
            KeyModifiers::CONTROL
        )));
    }
}
