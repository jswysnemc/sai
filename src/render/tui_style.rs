use crate::config::TuiStyle;

#[cfg(not(test))]
use std::sync::atomic::{AtomicU8, Ordering};

#[cfg(not(test))]
static CONFIGURED: AtomicU8 = AtomicU8::new(0);
#[cfg(not(test))]
static OVERRIDE: AtomicU8 = AtomicU8::new(2);

#[cfg(test)]
thread_local! {
    static STATE: std::cell::Cell<(TuiStyle, Option<TuiStyle>)> = const {
        std::cell::Cell::new((TuiStyle::Classic, None))
    };
}

/// 【终端】【样式配置】同步配置偏好，保留当前进程显式选择。
/// 参数: `style` 为配置样式；返回: 无。
pub(crate) fn configure(style: TuiStyle) {
    #[cfg(not(test))]
    CONFIGURED.store(style as u8, Ordering::Relaxed);
    #[cfg(test)]
    STATE.with(|state| state.set((style, state.get().1)));
}

/// 【终端】【样式覆盖】设置启动选项或切换命令指定的会话样式。
/// 参数: `style` 为显式选择，空值表示使用配置；返回: 无。
pub(crate) fn set_override(style: Option<TuiStyle>) {
    #[cfg(not(test))]
    OVERRIDE.store(style.map_or(2, |value| value as u8), Ordering::Relaxed);
    #[cfg(test)]
    STATE.with(|state| state.set((state.get().0, style)));
}

/// 【终端】【样式读取】读取当前生效样式，显式选择优先于配置。
/// 参数: 无；返回: 当前展示样式。
pub(crate) fn current() -> TuiStyle {
    #[cfg(not(test))]
    {
        let selected = OVERRIDE.load(Ordering::Relaxed);
        let value = if selected == 2 {
            CONFIGURED.load(Ordering::Relaxed)
        } else {
            selected
        };
        if value == 1 {
            TuiStyle::Deepseek
        } else {
            TuiStyle::Classic
        }
    }
    #[cfg(test)]
    STATE.with(|state| {
        let (configured, selected) = state.get();
        selected.unwrap_or(configured)
    })
}

/// 【终端】【样式判断】判断是否启用 DeepSeek harness 展示。
/// 参数: 无；返回: 新样式是否生效。
pub(crate) fn is_deepseek() -> bool {
    current() == TuiStyle::Deepseek
}
