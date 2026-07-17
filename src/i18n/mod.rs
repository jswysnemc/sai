mod locale;

pub use locale::{apply_locale_override_from_args, locale, Locale};

/// 判断当前界面语言是否为中文。
///
/// 返回:
/// - 中文界面返回 `true`，否则返回 `false`
pub fn is_zh() -> bool {
    locale() == Locale::Zh
}

/// 按当前界面语言选择静态文本。
///
/// 参数:
/// - `en`: 英文文本
/// - `zh`: 简体中文文本
///
/// 返回:
/// - 与当前界面语言匹配的文本
pub fn text(en: &'static str, zh: &'static str) -> &'static str {
    locale().text(en, zh)
}
