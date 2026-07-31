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

/// 探测并缓存终端对键盘增强协议的支持情况。
///
/// 探测需要往返查询终端，进程内只做一次；必须在 raw mode 下调用，
/// 当前唯一入口（TerminalInputGuard 启用序列）满足该前提。
///
/// 返回:
/// - 终端是否支持 kitty 键盘增强协议
fn terminal_supports_enhancement() -> bool {
    use std::sync::OnceLock;
    static SUPPORTED: OnceLock<bool> = OnceLock::new();
    *SUPPORTED.get_or_init(|| crossterm::terminal::supports_keyboard_enhancement().unwrap_or(false))
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
