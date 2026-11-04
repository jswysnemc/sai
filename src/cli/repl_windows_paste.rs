#[cfg(windows)]
use crate::clipboard::{self, ClipboardPayload};
use std::time::{Duration, Instant};

mod burst;
#[cfg(test)]
mod burst_tests;
mod control;
#[cfg(windows)]
mod native;
#[cfg(all(windows, test))]
mod native_tests;
#[cfg(any(windows, test))]
mod replay;

const WINDOWS_PASTE_BURST_WINDOW: Duration = Duration::from_millis(250);

/// Windows 终端将多行剪贴板拆成普通按键时的粘贴状态。
#[derive(Clone, Debug, Default)]
pub(super) struct WindowsPasteState {
    burst: burst::PasteBurst,
    control: control::PasteControl,
    recent_chars: String,
    last_char_at: Option<Instant>,
    replay: String,
    replay_offset: usize,
    replay_confirmed: bool,
}

/// Windows 粘贴检测成功后需要替换的输入片段。
#[derive(Debug, Eq, PartialEq)]
pub(super) struct WindowsPasteMatch {
    pub(super) prefix_chars: usize,
    pub(super) text: String,
}

/// Windows 终端输入事件中可参与粘贴回放匹配的按键。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum WindowsPasteKey {
    Char(char),
    Enter,
    LineFeed,
    Tab,
}

impl WindowsPasteState {
    /// 【终端】【粘贴保护】判断是否保护尚未确认的连续输入控制键
    /// 参数: now 为按键时间；返回控制键属于当前连续输入时为真
    pub(super) fn should_protect_control(&self, now: Instant) -> bool {
        self.burst.should_protect_control(now)
    }

    /// 【终端】【粘贴保护】将连续粘贴中的控制键写入输入框，避免换行触发提交
    /// 参数: input 为正文，cursor 为字符光标，key 为控制键，now 为事件时间
    /// 返回: 控制键属于粘贴且已插入时为真
    pub(super) fn insert_burst_control(
        &mut self,
        input: &mut String,
        cursor: &mut usize,
        key: WindowsPasteKey,
        now: Instant,
    ) -> bool {
        let ch = match key {
            WindowsPasteKey::Enter | WindowsPasteKey::LineFeed => '\n',
            WindowsPasteKey::Tab => '\t',
            WindowsPasteKey::Char(_) => return false,
        };
        if self.control.consume_paired_lf(key, now) {
            return true;
        }
        if !self.burst.consume_control(now) {
            return false;
        }
        self.control.record_newline(key, true, now);
        // 1. 【终端】【粘贴保护】字符光标转换为字节位置，保留光标之后已有正文
        let byte = input
            .char_indices()
            .nth(*cursor)
            .map_or(input.len(), |(byte, _)| byte);
        input.insert(byte, ch);
        *cursor += 1;
        true
    }

    /// 【终端】【Windows 粘贴】批量消费已确认属于剪贴板的控制台事件
    /// 参数: 无；返回本次消费的原生事件数，其他平台或接口不可用时返回零
    pub(super) fn drain_console_replay(&mut self) -> usize {
        #[cfg(windows)]
        {
            native::drain(self).unwrap_or_default()
        }
        #[cfg(not(windows))]
        {
            0
        }
    }

    /// 记录一个普通字符，为后续 Windows 粘贴识别保留短时突发输入。
    ///
    /// 参数:
    /// - `ch`: 当前字符
    /// - `now`: 当前事件时间
    ///
    /// 返回:
    /// - 无
    #[cfg(test)]
    pub(super) fn record_char(&mut self, ch: char, now: Instant) {
        self.record_text(ch.encode_utf8(&mut [0; 4]), now);
    }

    /// 【终端】【Windows 粘贴】一次登记同批文字，避免重复处理事件时间和输入缓冲。
    /// 参数: `text` 为连续输入文字，`now` 为接收时间
    /// 返回: 无
    pub(super) fn record_text(&mut self, text: &str, now: Instant) {
        self.burst.record_text(text, now);
        // 1. 相邻字符间隔过长时开始新的候选片段
        if self
            .last_char_at
            .is_some_and(|previous| now.duration_since(previous) > WINDOWS_PASTE_BURST_WINDOW)
        {
            self.recent_chars.clear();
        }
        // 2. 保留当前连续输入，长首行不受总耗时限制
        self.last_char_at = Some(now);
        self.recent_chars.push_str(text);
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
    #[cfg(test)]
    pub(super) fn begin_multiline(
        &mut self,
        input_before_cursor: &str,
        clipboard_text: &str,
        now: Instant,
    ) -> Option<WindowsPasteMatch> {
        self.begin_multiline_after(
            input_before_cursor,
            clipboard_text,
            WindowsPasteKey::Enter,
            now,
        )
    }

    /// 【终端】【粘贴识别】保留原始 CRLF 回放，插入正文统一使用 LF
    /// 参数: input_before_cursor 为光标前正文，clipboard_text 为原文，key 为首个换行键，
    /// now 为接收时间；返回匹配成功后的替换信息
    fn begin_multiline_after(
        &mut self,
        input_before_cursor: &str,
        clipboard_text: &str,
        key: WindowsPasteKey,
        now: Instant,
    ) -> Option<WindowsPasteMatch> {
        // 1. 换行必须紧跟连续字符，普通提交不读取为粘贴
        let previous = self.last_char_at?;
        if now.duration_since(previous) > WINDOWS_PASTE_BURST_WINDOW {
            self.reset_clipboard_candidate();
            return None;
        }

        // 2. 当前输入、连续字符和剪贴板首行必须完全对应
        let normalized = normalize_clipboard_text(clipboard_text);
        let Some(separator) = normalized.find('\n') else {
            self.reset_clipboard_candidate();
            return None;
        };
        let first_line = &normalized[..separator];
        if first_line.is_empty()
            || !input_before_cursor.ends_with(first_line)
            || !self.recent_chars.ends_with(first_line)
        {
            self.reset_clipboard_candidate();
            return None;
        }

        // 3. 保存首个换行之后的事件序列，后续按键只做匹配消费
        self.recent_chars.clear();
        self.last_char_at = None;
        let raw_separator = clipboard_text.find(['\r', '\n'])?;
        let newline_bytes = control::newline_bytes(&clipboard_text[raw_separator..])?;
        self.replay = clipboard_text[raw_separator + newline_bytes..].to_string();
        self.replay_offset = 0;
        self.replay_confirmed = true;
        self.control.record_newline(key, newline_bytes == 2, now);
        self.finish_replay_if_done(now);
        Some(WindowsPasteMatch {
            prefix_chars: first_line.chars().count(),
            text: normalized.trim().to_string(),
        })
    }

    /// 【终端】【粘贴识别】通过指定读取器识别剪贴板，供真实输入循环与隔离回归共用
    /// 参数: input_before_cursor 为光标前正文，key 为首个换行键，now 为事件时间，read 为剪贴板读取器
    /// 返回: 识别成功时的替换信息
    pub(super) fn begin_from_clipboard(
        &mut self,
        input_before_cursor: &str,
        key: WindowsPasteKey,
        now: Instant,
        read: impl FnOnce() -> Option<String>,
    ) -> Option<WindowsPasteMatch> {
        if self.last_char_at.is_none()
            || self
                .last_char_at
                .is_some_and(|previous| now.duration_since(previous) > WINDOWS_PASTE_BURST_WINDOW)
        {
            self.reset_clipboard_candidate();
            return None;
        }
        let Some(text) = read() else {
            self.reset_clipboard_candidate();
            return None;
        };
        self.begin_multiline_after(input_before_cursor, &text, key, now)
    }

    /// 消费已识别粘贴的后续按键，避免剩余换行再次触发提交。
    ///
    /// 参数:
    /// - `key`: 当前终端按键
    ///
    /// 返回:
    /// - 当前按键属于粘贴回放时返回 true
    #[cfg(test)]
    pub(super) fn consume_key(&mut self, key: WindowsPasteKey) -> bool {
        self.consume_key_at(key, Instant::now())
    }

    /// 【终端】【粘贴回放】使用接收时间消费普通字符、带修饰 LF 和可选 CRLF 配对
    /// 参数: key 为已分类的键，now 为接收时间；返回事件已消费时为真
    pub(super) fn consume_key_at(&mut self, key: WindowsPasteKey, now: Instant) -> bool {
        let finished = self.replay_confirmed && self.replay_offset == self.replay.len();
        let Some(bytes) = self
            .control
            .matching_bytes(&self.replay[self.replay_offset..], key, now)
        else {
            if finished {
                self.burst = burst::PasteBurst::default();
            }
            self.replay.clear();
            self.replay_offset = 0;
            self.replay_confirmed = false;
            self.control = control::PasteControl::default();
            return false;
        };
        // 1. 【终端】【Windows 粘贴】仅移动字节游标，避免逐字删除首部造成平方级搬移
        self.replay_offset += bytes;
        if self.replay_offset == self.replay.len() {
            self.replay.clear();
            self.replay_offset = 0;
        }
        self.finish_replay_if_done(now);
        true
    }

    /// 【终端】【粘贴结束】原文与可选 LF 全部消费后立即归还发送快捷键
    /// 参数: now 为消费时间；返回无
    fn finish_replay_if_done(&mut self, now: Instant) {
        if self.replay_confirmed && self.replay.is_empty() && !self.control.has_pending_lf(now) {
            self.replay_confirmed = false;
            self.burst = burst::PasteBurst::default();
        }
    }

    /// 清除当前粘贴识别状态。
    ///
    /// 返回:
    /// - 无
    pub(super) fn reset(&mut self) {
        self.burst = burst::PasteBurst::default();
        self.reset_clipboard_candidate();
    }

    /// 【终端】【粘贴保护】清除剪贴板匹配候选，保留独立的连续输入保护
    /// 参数: 无；返回无
    fn reset_clipboard_candidate(&mut self) {
        self.recent_chars.clear();
        self.last_char_at = None;
        self.control = control::PasteControl::default();
        self.replay.clear();
        self.replay_offset = 0;
        self.replay_confirmed = false;
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
pub(super) fn read_system_clipboard_text() -> Option<String> {
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
