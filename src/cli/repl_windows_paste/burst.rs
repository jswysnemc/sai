use std::time::{Duration, Instant};

const CHAR_INTERVAL: Duration = Duration::from_millis(8);
const CONTROL_SUPPRESS_WINDOW: Duration = Duration::from_millis(120);
const MIN_BURST_CHARS: usize = 3;

/// 【终端】【粘贴保护】根据连续字符时间识别没有括号粘贴标记的输入流
#[derive(Clone, Debug, Default)]
pub(super) struct PasteBurst {
    last_char_at: Option<Instant>,
    consecutive_chars: usize,
    suppress_until: Option<Instant>,
}

impl PasteBurst {
    /// 【终端】【粘贴保护】登记已批量接收的字符，延长粘贴控制键保护窗口
    /// 参数: text 为本批文字，now 为接收时间；返回无
    pub(super) fn record_text(&mut self, text: &str, now: Instant) {
        if text.is_empty() {
            return;
        }
        // 1. 【终端】【粘贴保护】普通键入间隔超过阈值时重新计数
        if !self
            .last_char_at
            .is_some_and(|previous| now.duration_since(previous) <= CHAR_INTERVAL)
        {
            self.consecutive_chars = 0;
        }
        self.last_char_at = Some(now);
        self.consecutive_chars = self.consecutive_chars.saturating_add(text.chars().count());
        // 2. 【终端】【粘贴保护】连续字符形成突发输入后，容许 Windows 后续事件短暂停顿
        if self.consecutive_chars >= MIN_BURST_CHARS {
            self.suppress_until = Some(now + CONTROL_SUPPRESS_WINDOW);
        }
    }

    /// 【终端】【粘贴保护】判定回车或制表符是否属于粘贴，并保护随后的空行
    /// 参数: now 为控制键时间；返回控制键应作为正文插入时为真
    pub(super) fn consume_control(&mut self, now: Instant) -> bool {
        if !self.should_protect_control(now) {
            return false;
        }
        self.suppress_until = Some(now + CONTROL_SUPPRESS_WINDOW);
        true
    }

    /// 【终端】【粘贴保护】只查询当前连续输入窗口，避免普通带修饰快捷键读取剪贴板
    /// 参数: now 为按键时间；返回当前控制键属于连续输入时为真
    pub(super) fn should_protect_control(&self, now: Instant) -> bool {
        let recent_char = self
            .last_char_at
            .is_some_and(|previous| now.duration_since(previous) <= CHAR_INTERVAL);
        let in_window = self.suppress_until.is_some_and(|until| now <= until);
        recent_char || in_window
    }
}
