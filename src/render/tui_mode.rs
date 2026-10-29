use crate::config::TuiMode;

#[cfg(not(test))]
use std::sync::atomic::{AtomicU8, Ordering};

#[cfg(not(test))]
static CONFIGURED: AtomicU8 = AtomicU8::new(1);
#[cfg(not(test))]
static OVERRIDE: AtomicU8 = AtomicU8::new(2);

#[cfg(test)]
thread_local! {
    static STATE: std::cell::Cell<(TuiMode, Option<TuiMode>)> = const {
        std::cell::Cell::new((TuiMode::Inline, None))
    };
}

/// 【终端】【样式配置】同步配置偏好，保留当前进程显式选择。
/// 参数: `style` 为配置样式；返回: 无。
pub(crate) fn configure(style: TuiMode) {
    #[cfg(not(test))]
    CONFIGURED.store(style as u8, Ordering::Relaxed);
    #[cfg(test)]
    STATE.with(|state| state.set((style, state.get().1)));
}

/// 【终端】【样式覆盖】设置启动选项或切换命令指定的会话样式。
/// 参数: `style` 为显式选择，空值表示使用配置；返回: 无。
pub(crate) fn set_override(style: Option<TuiMode>) {
    #[cfg(not(test))]
    OVERRIDE.store(style.map_or(2, |value| value as u8), Ordering::Relaxed);
    #[cfg(test)]
    STATE.with(|state| state.set((state.get().0, style)));
}

/// 【终端】【样式读取】读取当前生效样式，显式选择优先于配置。
/// 参数: 无；返回: 当前展示样式。
pub(crate) fn current() -> TuiMode {
    #[cfg(not(test))]
    {
        let selected = OVERRIDE.load(Ordering::Relaxed);
        let value = if selected == 2 {
            CONFIGURED.load(Ordering::Relaxed)
        } else {
            selected
        };
        if value == 1 {
            TuiMode::Fullscreen
        } else {
            TuiMode::Inline
        }
    }
    #[cfg(test)]
    STATE.with(|state| {
        let (configured, selected) = state.get();
        selected.unwrap_or(configured)
    })
}

/// 【终端】【启动模式】判断是否选择全屏启动，内联与全屏共用统一内容渲染。
/// 参数: 无；返回: 新样式是否生效。
pub(crate) fn prefers_fullscreen() -> bool {
    current() == TuiMode::Fullscreen
}
