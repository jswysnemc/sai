#[cfg(windows)]
use crate::clipboard::{self, ClipboardPayload};
use std::time::{Duration, Instant};

const WINDOWS_PASTE_BURST_WINDOW: Duration = Duration::from_millis(250);

/// Windows 终端将多行剪贴板拆成普通按键时的粘贴状态。
#[derive(Clone, Debug, Default)]
pub(super) struct WindowsPasteState {
    recent_chars: String,
    last_char_at: Option<Instant>,
    replay: String,
}

/// Windows 粘贴检测成功后需要替换的输入片段。
#[derive(Debug, Eq, PartialEq)]
pub(super) struct WindowsPasteMatch {
    pub(super) prefix_chars: usize,
    pub(super) text: String,
}

/// Windows 终端输入事件中可参与粘贴回放匹配的按键。
#[derive(Debug, Eq, PartialEq)]
pub(super) enum WindowsPasteKey {
    Char(char),
    Enter,
    Tab,
}

impl WindowsPasteState {
    /// 记录一个普通字符，为后续 Windows 粘贴识别保留短时突发输入。
    ///
    /// 参数:
    /// - `ch`: 当前字符
    /// - `now`: 当前事件时间
    ///
    /// 返回:
    /// - 无
    pub(super) fn record_char(&mut self, ch: char, now: Instant) {
        // 1. 相邻字符间隔过长时开始新的候选片段
        if self
            .last_char_at
            .is_some_and(|previous| now.duration_since(previous) > WINDOWS_PASTE_BURST_WINDOW)
        {
            self.recent_chars.clear();
        }
        // 2. 保留当前连续输入，长首行不受总耗时限制
        self.last_char_at = Some(now);
        self.recent_chars.push(ch);
    }

    /// 根据当前输入和系统剪贴板内容识别被拆开的多行粘贴。
    ///
    /// 参数:
    /// - `input_before_cursor`: 光标前的当前输入
    /// - `clipboard_text`: 系统剪贴板正文
    /// - `now`: 当前事件时间
    ///
    /// 返回:
    /// - 识别成功时的替换信息
    pub(super) fn begin_multiline(
        &mut self,
        input_before_cursor: &str,
        clipboard_text: &str,
        now: Instant,
    ) -> Option<WindowsPasteMatch> {
        // 1. 换行必须紧跟连续字符，普通提交不读取为粘贴
        let previous = self.last_char_at?;
        if now.duration_since(previous) > WINDOWS_PASTE_BURST_WINDOW {
            self.reset();
            return None;
        }

        // 2. 当前输入、连续字符和剪贴板首行必须完全对应
        let normalized = normalize_clipboard_text(clipboard_text);
        let Some(separator) = normalized.find('\n') else {
            self.reset();
            return None;
        };
        let first_line = &normalized[..separator];
        if first_line.is_empty()
            || !input_before_cursor.ends_with(first_line)
            || !self.recent_chars.ends_with(first_line)
        {
            self.reset();
            return None;
        }

        // 3. 保存首个换行之后的事件序列，后续按键只做匹配消费
        self.recent_chars.clear();
        self.last_char_at = None;
        self.replay = normalized[separator + 1..].to_string();
        Some(WindowsPasteMatch {
            prefix_chars: first_line.chars().count(),
            text: normalized.trim().to_string(),
        })
    }

    /// 读取系统剪贴板并尝试识别 Windows 拆分的多行粘贴。
    ///
    /// 参数:
    /// - `input_before_cursor`: 光标前的当前输入
    /// - `now`: 当前事件时间
    ///
    /// 返回:
    /// - 识别成功时的替换信息
    pub(super) fn begin_from_system_clipboard(
        &mut self,
        input_before_cursor: &str,
        now: Instant,
    ) -> Option<WindowsPasteMatch> {
        if self.last_char_at.is_none()
            || self
                .last_char_at
                .is_some_and(|previous| now.duration_since(previous) > WINDOWS_PASTE_BURST_WINDOW)
        {
            self.reset();
            return None;
        }
        let Some(text) = read_system_clipboard_text() else {
            self.reset();
            return None;
        };
        self.begin_multiline(input_before_cursor, &text, now)
    }

    /// 消费已识别粘贴的后续按键，避免剩余换行再次触发提交。
    ///
    /// 参数:
    /// - `key`: 当前终端按键
    ///
    /// 返回:
    /// - 当前按键属于粘贴回放时返回 true
    pub(super) fn consume_key(&mut self, key: WindowsPasteKey) -> bool {
        let Some(expected) = self.replay.chars().next() else {
            return false;
        };
        let actual = match key {
            WindowsPasteKey::Char(ch) => ch,
            WindowsPasteKey::Enter => '\n',
            WindowsPasteKey::Tab => '\t',
        };
        if actual != expected {
            self.replay.clear();
            return false;
        }
        let next = expected.len_utf8();
        self.replay.drain(..next);
        true
    }

    /// 清除当前粘贴识别状态。
    ///
    /// 返回:
    /// - 无
    pub(super) fn reset(&mut self) {
        self.recent_chars.clear();
        self.last_char_at = None;
        self.replay.clear();
    }
}

/// 统一 Windows 粘贴事件的换行格式。
///
/// 参数:
/// - `text`: 系统剪贴板正文
///
/// 返回:
/// - 使用 LF 换行的正文
fn normalize_clipboard_text(text: &str) -> String {
    text.replace("\r\n", "\n").replace('\r', "\n")
}

/// 读取 Windows 系统剪贴板中的文本。
///
/// 返回:
/// - Windows 文本剪贴板正文，其他平台返回空值
fn read_system_clipboard_text() -> Option<String> {
    #[cfg(windows)]
    {
        let payload = clipboard::read_clipboard_payload().ok()?;
        let ClipboardPayload::Text(text) = payload else {
            return None;
        };
        Some(text)
    }
    #[cfg(not(windows))]
    {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn groups_windows_multiline_key_stream_into_one_atomic_paste() {
        let text = "第一行\n第二行\n第三行";
        let first_line = "第一行";
        let start = Instant::now();
        let mut state = WindowsPasteState::default();

        for (index, ch) in first_line.chars().enumerate() {
            state.record_char(ch, start + Duration::from_millis(index as u64));
        }

        let matched =
            state.begin_multiline("问题：第一行", text, start + Duration::from_millis(20));

        assert_eq!(
            matched,
            Some(WindowsPasteMatch {
                prefix_chars: first_line.chars().count(),
                text: text.to_string()
            })
        );
        assert!(state.consume_key(WindowsPasteKey::Char('第')));
        assert!(state.consume_key(WindowsPasteKey::Char('二')));
        assert!(state.consume_key(WindowsPasteKey::Char('行')));
        assert!(state.consume_key(WindowsPasteKey::Enter));
        assert!(state.consume_key(WindowsPasteKey::Char('第')));
        assert!(state.consume_key(WindowsPasteKey::Char('三')));
        assert!(state.consume_key(WindowsPasteKey::Char('行')));
        assert!(!state.consume_key(WindowsPasteKey::Enter));
    }

    #[test]
    fn keeps_long_first_line_candidate_alive_while_events_continue() {
        let start = Instant::now();
        let mut state = WindowsPasteState::default();
        let first_line = "a".repeat(320);

        for (index, ch) in first_line.chars().enumerate() {
            state.record_char(ch, start + Duration::from_millis(index as u64 * 2));
        }

        let matched = state.begin_multiline(
            &first_line,
            &format!("{first_line}\nsecond"),
            start + Duration::from_millis(641),
        );

        assert!(matched.is_some());
    }

    #[test]
    fn does_not_replace_normal_submission_after_input_pause() {
        let start = Instant::now();
        let mut state = WindowsPasteState::default();
        for ch in "first".chars() {
            state.record_char(ch, start);
        }

        let matched = state.begin_multiline(
            "first",
            "first\nsecond",
            start + WINDOWS_PASTE_BURST_WINDOW + Duration::from_millis(1),
        );

        assert!(matched.is_none());
    }

    #[test]
    fn does_not_replace_input_that_differs_from_clipboard_first_line() {
        let start = Instant::now();
        let mut state = WindowsPasteState::default();
        for ch in "typed".chars() {
            state.record_char(ch, start);
        }

        let matched = state.begin_multiline("typed", "clipboard\nsecond", start);

        assert!(matched.is_none());
    }

    #[test]
    fn preserves_raw_replay_but_trims_atomic_payload() {
        let start = Instant::now();
        let mut state = WindowsPasteState::default();
        for ch in "  first  ".chars() {
            state.record_char(ch, start);
        }

        let matched = state
            .begin_multiline("prefix  first  ", "  first  \r\nsecond\r\n", start)
            .expect("应识别带缩进与 CRLF 的粘贴");

        assert_eq!(matched.prefix_chars, "  first  ".chars().count());
        assert_eq!(matched.text, "first  \nsecond");
        for key in [
            WindowsPasteKey::Char('s'),
            WindowsPasteKey::Char('e'),
            WindowsPasteKey::Char('c'),
            WindowsPasteKey::Char('o'),
            WindowsPasteKey::Char('n'),
            WindowsPasteKey::Char('d'),
            WindowsPasteKey::Enter,
        ] {
            assert!(state.consume_key(key));
        }
        assert!(!state.consume_key(WindowsPasteKey::Enter));
    }
}
