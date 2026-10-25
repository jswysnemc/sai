use crate::i18n::{with_locale as scoped_locale, Locale};

/// 【终端】【展示语言】DeepSeek 渲染使用英文，经典样式沿用当前语言，不修改进程设置。
/// 参数: `render` 为同步渲染操作；返回: 渲染结果，结束或异常展开后恢复原语言。
pub(crate) fn with_locale<Output>(render: impl FnOnce() -> Output) -> Output {
    if super::tui_style::is_deepseek() {
        scoped_locale(Locale::En, render)
    } else {
        render()
    }
}

#[cfg(test)]
#[path = "tui_presentation_tests.rs"]
mod tests;
