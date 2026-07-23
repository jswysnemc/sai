use crossterm::event::{
    KeyboardEnhancementFlags, PopKeyboardEnhancementFlags, PushKeyboardEnhancementFlags,
};
use crossterm::execute;
use std::io::Write;

/// 键盘增强协议的平台执行策略。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum KeyboardEnhancementStrategy {
    BestEffort,
    Skip,
}

/// 根据目标平台选择键盘增强协议策略。
///
/// 参数:
/// - `is_windows`: 目标平台是否为 Windows
///
/// 返回:
/// - 目标平台应采用的键盘增强协议策略
pub(super) const fn strategy_for_platform(is_windows: bool) -> KeyboardEnhancementStrategy {
    if is_windows {
        KeyboardEnhancementStrategy::Skip
    } else {
        KeyboardEnhancementStrategy::BestEffort
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

        let active = execute!(
            writer,
            PushKeyboardEnhancementFlags(KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES)
        )
        .is_ok();
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
            let _ = execute!(writer, PopKeyboardEnhancementFlags);
            self.active = false;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{strategy_for_platform, KeyboardEnhancementStrategy};

    /// Windows 旧式控制台必须跳过未实现的键盘增强协议。
    #[test]
    fn legacy_windows_skips_keyboard_enhancement() {
        assert_eq!(
            strategy_for_platform(true),
            KeyboardEnhancementStrategy::Skip
        );
    }

    /// 非 Windows 终端继续以可降级方式启用键盘增强协议。
    #[test]
    fn non_windows_uses_best_effort_keyboard_enhancement() {
        assert_eq!(
            strategy_for_platform(false),
            KeyboardEnhancementStrategy::BestEffort
        );
    }
}
