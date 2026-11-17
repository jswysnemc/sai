//! 配置界面的键盘、鼠标、粘贴与终端尺寸事件读取及未保存中断保护。

use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseEvent};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

/// 当前处于未保存更改保护状态的嵌套作用域计数。
static ACTIVE_UNSAVED_SCOPES: AtomicUsize = AtomicUsize::new(0);

/// 【配置界面】【输入事件】统一承载键盘、鼠标、括号粘贴与终端尺寸变更事件。
#[derive(Clone, Debug)]
pub(crate) enum ConfigEvent {
    Key(KeyEvent),
    Mouse(MouseEvent),
    Paste(String),
    Resize,
}

/// 【配置界面】【未保存守卫】按作用域引用计数追踪未保存状态，退出或异常展开时自动释放。
#[derive(Debug, Default)]
pub(crate) struct UnsavedState {
    active: bool,
}

impl UnsavedState {
    /// 【配置界面】【状态同步】更新当前作用域是否存在未落盘修改。
    ///
    /// 参数:
    /// - `unsaved`: 当前作用域是否有未保存改动
    ///
    /// 返回:
    /// - 无
    pub(crate) fn set(&mut self, unsaved: bool) {
        if unsaved && !self.active {
            self.active = true;
            ACTIVE_UNSAVED_SCOPES.fetch_add(1, Ordering::SeqCst);
        } else if !unsaved && self.active {
            self.active = false;
            // 1. 【配置界面】【原子饱和递减】采用 compare_exchange_weak 循环替代已弃用的 fetch_update，兼容多版本工具链且保持零警告
            let mut current = ACTIVE_UNSAVED_SCOPES.load(Ordering::SeqCst);
            while let Err(actual) = ACTIVE_UNSAVED_SCOPES.compare_exchange_weak(
                current,
                current.saturating_sub(1),
                Ordering::SeqCst,
                Ordering::SeqCst,
            ) {
                current = actual;
            }
        }
    }
}

impl Drop for UnsavedState {
    /// 【配置界面】【守卫释放】作用域结束时归还引用计数，避免污染后续会话。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 无
    fn drop(&mut self) {
        self.set(false);
    }
}

/// 【配置界面】【未保存查询】判断当前是否存在任何待确认的未保存更改。
///
/// 参数:
/// - 无
///
/// 返回:
/// - 任一活跃作用域存在未保存修改时返回 true
pub(crate) fn has_unsaved_changes() -> bool {
    ACTIVE_UNSAVED_SCOPES.load(Ordering::SeqCst) > 0
}

/// 【配置界面】【单行粘贴净化】将粘贴文本中的换行与控制字符折叠为空格，适配单行输入框与搜索栏。
///
/// 参数:
/// - `text`: 原始粘贴文本
///
/// 返回:
/// - 去除控制字符后的单行文本
pub(crate) fn sanitize_paste_single_line(text: &str) -> String {
    let trimmed = text.trim_matches(['\r', '\n']);
    let mut out = String::with_capacity(trimmed.len());
    let mut prev_space = false;
    for ch in trimmed.chars() {
        if matches!(ch, '\r' | '\n' | '\t') {
            if !prev_space && !out.is_empty() {
                out.push(' ');
                prev_space = true;
            }
            continue;
        }
        if ch.is_control() {
            continue;
        }
        prev_space = ch == ' ';
        out.push(ch);
    }
    out
}

/// Ctrl+C / Ctrl+D 中断信号。
#[derive(Debug)]
pub(crate) struct Interrupted;

impl std::fmt::Display for Interrupted {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("interrupted")
    }
}

impl std::error::Error for Interrupted {}

/// 【配置界面】【中断识别】判断按键事件是否为 Ctrl+C 或 Ctrl+D 中断组合键。
///
/// 参数:
/// - `event`: 终端键盘事件
///
/// 返回:
/// - 命中中断组合键时返回 true
fn is_interrupt(event: KeyEvent) -> bool {
    matches!(
        (event.code, event.modifiers.contains(KeyModifiers::CONTROL)),
        (
            KeyCode::Char('c') | KeyCode::Char('C') | KeyCode::Char('d') | KeyCode::Char('D'),
            true
        )
    )
}

/// 【配置界面】【按键归一化】过滤释放事件并在存在未保存改动时将 Ctrl+C 转为安全返回键。
///
/// 参数:
/// - `event`: 原始终端键盘事件
///
/// 返回:
/// - 可驱动界面的按键事件；释放事件返回 `Ok(None)`；无改动下按中断键返回 `Err(Interrupted)`
fn resolve_key_event(event: KeyEvent) -> Result<Option<KeyEvent>> {
    if !is_actionable_key_event(event) {
        return Ok(None);
    }
    if is_interrupt(event) {
        // 1. 【配置界面】【未保存保护】有未保存更改时将中断转为 Esc，引导进入确认弹窗而非静默丢弃修改
        if has_unsaved_changes() {
            return Ok(Some(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)));
        }
        return Err(Interrupted.into());
    }
    Ok(Some(event))
}

/// 【配置界面】【事件读取】阻塞读取键盘、鼠标、粘贴或尺寸调整事件。
///
/// 参数:
/// - 无
///
/// 返回:
/// - 归一化的配置界面输入事件
pub(crate) fn read_event() -> Result<ConfigEvent> {
    read_event_with_timeout(None).map(|event| event.expect("blocking read should return an event"))
}

/// 【配置界面】【超时事件】读取键盘、鼠标、粘贴或尺寸调整事件，可选等待超时。
///
/// 参数:
/// - `timeout`: 等待终端事件的时长；为空时持续阻塞等待
///
/// 返回:
/// - 归一化输入事件；超时无事件时返回空
pub(crate) fn read_event_with_timeout(timeout: Option<Duration>) -> Result<Option<ConfigEvent>> {
    loop {
        // 1. 【配置界面】【超时轮询】按需等待终端输入，超时后返回空值供后台任务刷新
        if let Some(timeout) = timeout {
            if !event::poll(timeout)? {
                return Ok(None);
            }
        }
        // 2. 【配置界面】【事件分发】统一转换按键、鼠标、粘贴与尺寸变更事件
        match event::read()? {
            Event::Key(event) => {
                if let Some(resolved) = resolve_key_event(event)? {
                    return Ok(Some(ConfigEvent::Key(resolved)));
                }
            }
            Event::Mouse(mouse) => return Ok(Some(ConfigEvent::Mouse(mouse))),
            Event::Paste(text) => return Ok(Some(ConfigEvent::Paste(text))),
            Event::Resize(_, _) => return Ok(Some(ConfigEvent::Resize)),
            _ => {}
        }
    }
}

/// 【配置界面】【按下判定】判断键盘事件是否属于按下或长按重复事件。
///
/// 参数:
/// - `event`: 终端键盘事件
///
/// 返回:
/// - 非释放态按键返回 true
fn is_actionable_key_event(event: KeyEvent) -> bool {
    event.kind != KeyEventKind::Release
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{KeyEventKind, KeyEventState, KeyModifiers};
    use std::sync::Mutex;

    static TEST_LOCK: Mutex<()> = Mutex::new(());

    /// 【配置界面】【释放过滤测试】验证忽略控制台残留的按键释放事件。
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

    /// 【配置界面】【中断识别测试】验证 Ctrl+C 与 Ctrl+D 被识别为中断键。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 无
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

    /// 【配置界面】【未保存中断保护测试】验证存在未保存改动时 Ctrl+C 转为 Esc 而非直接中断退出。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 无
    #[test]
    fn unsaved_scope_converts_ctrl_c_to_escape_and_restores_on_drop() {
        let _lock = TEST_LOCK.lock().unwrap();
        let ctrl_c = KeyEvent {
            code: KeyCode::Char('c'),
            modifiers: KeyModifiers::CONTROL,
            kind: KeyEventKind::Press,
            state: KeyEventState::NONE,
        };

        assert!(resolve_key_event(ctrl_c).is_err());
        {
            let mut guard = UnsavedState::default();
            guard.set(true);
            let mapped = resolve_key_event(ctrl_c).unwrap().unwrap();
            assert_eq!(mapped.code, KeyCode::Esc);
        }
        assert!(!has_unsaved_changes());
        assert!(resolve_key_event(ctrl_c).is_err());
    }

    /// 【配置界面】【粘贴净化测试】验证多行粘贴文本被净化为单行安全字符。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 无
    #[test]
    fn sanitize_paste_collapses_newlines_and_strips_controls() {
        assert_eq!(
            sanitize_paste_single_line("gpt-4o\r\nmini\x07"),
            "gpt-4o mini"
        );
    }
}
