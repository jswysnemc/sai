//! 终端键盘增强协议（Kitty Keyboard Protocol）探测与生命周期管理。
//!
//! 在 Unix 与 Windows 现代 ConPTY 终端（如 ELYTerminal、Winc）上均先通过
//! `\x1b[?u\x1b[c` 真实探测支持情况，仅在终端明确返回 `\x1b[?<flags>u` 时才压入
//! Level 1（`DISAMBIGUATE_ESCAPE_CODES`），不支持的宿主保持零副作用。

use std::io::Write;

#[cfg(any(windows, test))]
const KITTY_KEYBOARD_PREFIX: &str = "\x1b[?";
#[cfg(any(windows, test))]
const MAX_PROBE_BUFFER: usize = 128;

/// 键盘增强协议的平台执行策略。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum KeyboardEnhancementStrategy {
    BestEffort,
    #[allow(dead_code)]
    Skip,
}

/// 根据目标平台选择键盘增强协议策略。
///
/// Windows 通过原生 `ConsoleQuery` 主动探测 Kitty 键盘支持，
/// 不支持的旧式控制台与 Windows Terminal 会在 DA1 应答时快速跳过。
///
/// 参数:
/// - `_is_windows`: 目标平台是否为 Windows
///
/// 返回:
/// - 目标平台应采用的键盘增强协议策略
pub(super) const fn strategy_for_platform(_is_windows: bool) -> KeyboardEnhancementStrategy {
    KeyboardEnhancementStrategy::BestEffort
}

/// 探测并缓存终端对键盘增强协议的支持情况。
///
/// 探测需要往返查询终端，进程内只做一次；必须在 raw mode 下调用，
/// 当前唯一入口（TerminalInputGuard 启用序列）满足该前提。
///
/// 返回:
/// - 终端是否支持 kitty 键盘增强协议
fn terminal_supports_enhancement() -> bool {
    if cfg!(test) {
        return false;
    }
    use std::sync::OnceLock;
    static SUPPORTED: OnceLock<bool> = OnceLock::new();
    *SUPPORTED.get_or_init(probe_terminal_keyboard_enhancement)
}

/// 【终端】【键盘探测】按平台执行一次有界 Kitty 键盘能力探测。
///
/// 参数:
/// - 无
///
/// 返回:
/// - 终端明确支持 Kitty 键盘协议时返回 true
fn probe_terminal_keyboard_enhancement() -> bool {
    #[cfg(windows)]
    {
        probe_windows_keyboard_enhancement().unwrap_or(false)
    }
    #[cfg(not(windows))]
    {
        crossterm::terminal::supports_keyboard_enhancement().unwrap_or(false)
    }
}

/// 【终端】【Windows 键盘探测】经由 ConPTY 原生输入记录发送 `\x1b[?u\x1b[c` 验证 Kitty 键盘支持。
///
/// 参数:
/// - 无
///
/// 返回:
/// - 终端返回 `\x1b[?<flags>u` 时为 true，普通用户输入始终完整归还控制台
#[cfg(windows)]
fn probe_windows_keyboard_enhancement() -> std::io::Result<bool> {
    use crate::platform::windows_graphics_probe::{response_fragment, ConsoleQuery};
    use std::io::IsTerminal;
    use std::time::{Duration, Instant};

    if !std::io::stdin().is_terminal() || !std::io::stdout().is_terminal() {
        return Ok(false);
    }
    let console = ConsoleQuery::open()?;
    let mut parser = KeyboardProbeParser::default();
    let result = (|| -> std::io::Result<bool> {
        let mut stdout = std::io::stdout();
        stdout.write_all(b"\x1b[?u\x1b[c")?;
        stdout.flush()?;
        let deadline = Instant::now() + Duration::from_millis(150);
        while let Some(remaining) = deadline.checked_duration_since(Instant::now()) {
            let Some(record) = console.read(remaining)? else {
                break;
            };
            if let Some(done) = parser.push(response_fragment(&record), record) {
                return Ok(done);
            }
        }
        Ok(parser.supported())
    })();
    let restored = console.restore_input(parser.into_input());
    restored?;
    result
}

/// 【终端】【键盘探测解析器】从输入流中识别 `\x1b[?<flags>u` 与 DA1 哨兵 `\x1b[?...c`，保留穿插用户输入。
#[cfg(any(windows, test))]
#[derive(Default)]
struct KeyboardProbeParser<T> {
    supported: bool,
    text: String,
    pending: Vec<usize>,
    input: Vec<Option<T>>,
}

#[cfg(any(windows, test))]
impl<T> KeyboardProbeParser<T> {
    /// 推入一条输入片段与原始记录。
    ///
    /// 参数:
    /// - `fragment`: 可参与协议匹配的 ASCII 片段
    /// - `record`: 原始输入事件或记录
    ///
    /// 返回:
    /// - 收到 DA1 哨兵时返回 `Some(是否支持 Kitty 键盘协议)`，继续等待时返回 None
    fn push(&mut self, fragment: Option<String>, record: T) -> Option<bool> {
        let index = self.input.len();
        self.input.push(Some(record));
        let Some(fragment) = fragment else {
            return None;
        };
        if fragment.starts_with('\x1b') && !self.text.is_empty() {
            self.reset_pending();
        }
        self.text.push_str(&fragment);
        self.pending.push(index);
        if KITTY_KEYBOARD_PREFIX.starts_with(&self.text) {
            return None;
        }
        if let Some(body) = self.text.strip_prefix(KITTY_KEYBOARD_PREFIX) {
            if let Some(flags) = body.strip_suffix('u') {
                if !flags.is_empty() && flags.bytes().all(|byte| byte.is_ascii_digit()) {
                    self.supported = true;
                }
                self.discard_pending();
                return None;
            }
            if let Some(attrs) = body.strip_suffix('c') {
                let valid_da1 = !attrs.is_empty()
                    && attrs
                        .bytes()
                        .all(|byte| byte.is_ascii_digit() || byte == b';');
                self.discard_pending();
                if valid_da1 {
                    return Some(self.supported);
                }
                return None;
            }
            if self.text.len() <= MAX_PROBE_BUFFER
                && body
                    .bytes()
                    .all(|byte| byte.is_ascii_digit() || byte == b';')
            {
                return None;
            }
        }
        self.reset_pending();
        None
    }

    /// 返回目前已识别的支持状态。
    fn supported(&self) -> bool {
        self.supported
    }

    /// 结束探测并返回所有普通用户输入。
    fn into_input(mut self) -> Vec<T> {
        if self.text.starts_with(KITTY_KEYBOARD_PREFIX) {
            self.discard_pending();
        }
        self.reset_pending();
        self.input.into_iter().flatten().collect()
    }

    fn reset_pending(&mut self) {
        self.pending.clear();
        self.text.clear();
    }

    fn discard_pending(&mut self) {
        for index in self.pending.drain(..) {
            self.input[index] = None;
        }
        self.text.clear();
    }
}

/// 记录当前终端是否成功启用了键盘增强协议。
#[derive(Debug, Default)]
pub(super) struct KeyboardEnhancementState {
    active: bool,
}

impl KeyboardEnhancementState {
    /// 尝试启用键盘增强协议，不支持时保持普通键盘输入。
    ///
    /// 参数:
    /// - `writer`: 接收终端控制序列的输出流
    ///
    /// 返回:
    /// - 是否成功启用协议的状态对象
    pub(super) fn enable<W: Write>(writer: &mut W) -> Self {
        if strategy_for_platform(cfg!(windows)) == KeyboardEnhancementStrategy::Skip {
            return Self::default();
        }
        // 真实探测终端能力：写入 push 序列几乎总是"成功"，
        // 不能作为支持与否的依据；不支持的终端不写入任何协议序列
        if !terminal_supports_enhancement() {
            return Self::default();
        }
        // 经切屏管理计数：切到备用屏时这一层随之迁移，出栈落在同一块屏幕上
        let active = super::alternate_screen::push_enhancement(writer).is_ok();
        Self { active }
    }

    /// 恢复启用前的键盘增强协议状态。
    ///
    /// 参数:
    /// - `writer`: 接收终端控制序列的输出流
    ///
    /// 返回:
    /// - 无
    pub(super) fn disable<W: Write>(&mut self, writer: &mut W) {
        if self.active {
            let _ = super::alternate_screen::pop_enhancement(writer);
            self.active = false;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{strategy_for_platform, KeyboardEnhancementStrategy, KeyboardProbeParser};

    /// Windows 与非 Windows 均采用按探测结果可降级启用的策略。
    #[test]
    fn both_platforms_use_best_effort_keyboard_enhancement() {
        assert_eq!(
            strategy_for_platform(true),
            KeyboardEnhancementStrategy::BestEffort
        );
        assert_eq!(
            strategy_for_platform(false),
            KeyboardEnhancementStrategy::BestEffort
        );
    }

    /// 【终端测试】【Kitty 键盘探测】识别 `\x1b[?0u` 与 DA1 哨兵并保留穿插用户输入。
    #[test]
    fn keyboard_probe_parser_detects_kitty_reply_and_preserves_user_input() {
        let mut parser = KeyboardProbeParser::<char>::default();
        let stream = "a\x1b[?0ub\x1b[?62;4c";
        let mut result = None;
        for ch in stream.chars() {
            result = parser.push(Some(ch.to_string()), ch);
        }
        assert_eq!(result, Some(true));
        assert_eq!(parser.into_input(), vec!['a', 'b']);
    }

    /// 【终端测试】【不支持终端】仅收到 DA1 时立刻返回 false，避免等待超时。
    #[test]
    fn keyboard_probe_parser_returns_false_on_da1_without_kitty_reply() {
        let mut parser = KeyboardProbeParser::<char>::default();
        let mut result = None;
        for ch in "\x1b[?1;0c".chars() {
            result = parser.push(Some(ch.to_string()), ch);
        }
        assert_eq!(result, Some(false));
        assert!(!parser.supported());
        assert!(parser.into_input().is_empty());
    }

    /// 【终端测试】【超时保底】收到 `\x1b[?0u` 但未等到 DA1 时仍返回已识别到的支持状态。
    #[test]
    fn keyboard_probe_parser_retains_detected_support_if_da1_times_out() {
        let mut parser = KeyboardProbeParser::<char>::default();
        for ch in "\x1b[?0u".chars() {
            assert_eq!(parser.push(Some(ch.to_string()), ch), None);
        }
        assert!(parser.supported());
        assert!(parser.into_input().is_empty());
    }
}
