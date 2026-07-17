use std::ffi::OsString;
use std::sync::atomic::{AtomicU8, Ordering};

const LOCALE_AUTO: u8 = 0;
const LOCALE_EN_US: u8 = 1;
const LOCALE_ZH_CN: u8 = 2;

static LOCALE_OVERRIDE: AtomicU8 = AtomicU8::new(LOCALE_AUTO);

/// Sai 支持的界面语言。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Locale {
    /// 美式英文。
    En,
    /// 简体中文。
    Zh,
}

impl Locale {
    /// 从显式语言代码解析受支持语言。
    ///
    /// 参数:
    /// - `value`: 语言代码，例如 `en`、`en-US`、`zh` 或 `zh-CN`
    ///
    /// 返回:
    /// - 支持的语言；无法识别时返回空
    pub fn parse(value: &str) -> Option<Self> {
        let normalized = value.trim().replace('_', "-").to_ascii_lowercase();
        match normalized.as_str() {
            "en" | "en-us" => Some(Self::En),
            "zh" | "zh-cn" | "zh-hans" | "zh-sg" | "zh-tw" | "zh-hk" | "zh-hant" => Some(Self::Zh),
            _ => None,
        }
    }

    /// 根据环境变量检测界面语言。
    ///
    /// 检测顺序为 `SAI_LANG`、`LC_ALL`、`LC_MESSAGES`、`LANG`。
    ///
    /// 返回:
    /// - 检测到的受支持语言；没有明确中文环境时返回英文
    pub fn detect() -> Self {
        let values = ["SAI_LANG", "LC_ALL", "LC_MESSAGES", "LANG"]
            .into_iter()
            .filter_map(|key| std::env::var(key).ok())
            .collect::<Vec<_>>();
        detect_from_locale_values(values.iter().map(String::as_str))
    }

    /// 返回用于配置和命令行展示的标准语言代码。
    ///
    /// 返回:
    /// - `en-US` 或 `zh-CN`
    pub fn code(self) -> &'static str {
        match self {
            Self::En => "en-US",
            Self::Zh => "zh-CN",
        }
    }

    /// 按指定语言选择静态文本。
    ///
    /// 参数:
    /// - `en`: 英文文本
    /// - `zh`: 简体中文文本
    ///
    /// 返回:
    /// - 与语言匹配的文本
    pub fn text(self, en: &'static str, zh: &'static str) -> &'static str {
        match self {
            Self::En => en,
            Self::Zh => zh,
        }
    }
}

/// 按优先顺序读取区域设置候选值。
///
/// 参数:
/// - `values`: 从高到低排列的区域设置值
///
/// 返回:
/// - 第一个受支持语言；所有值均不受支持时返回英文
fn detect_from_locale_values<'a>(values: impl IntoIterator<Item = &'a str>) -> Locale {
    for value in values {
        let code = value.trim().split(['.', '@']).next().unwrap_or_default();
        if code.eq_ignore_ascii_case("c") || code.eq_ignore_ascii_case("posix") {
            continue;
        }
        if let Some(locale) = Locale::parse(code) {
            return locale;
        }
    }
    Locale::En
}

/// 返回当前进程采用的界面语言。
///
/// 返回:
/// - 命令行覆盖语言；未设置覆盖时返回环境检测结果
pub fn locale() -> Locale {
    match LOCALE_OVERRIDE.load(Ordering::Relaxed) {
        LOCALE_EN_US => Locale::En,
        LOCALE_ZH_CN => Locale::Zh,
        _ => Locale::detect(),
    }
}

/// 在 Clap 构建帮助信息前读取 `--lang` 并设置进程级语言覆盖。
///
/// 参数:
/// - `args`: 原始命令行参数
///
/// 返回:
/// - 识别到的语言；未提供或参数无效时返回空
pub fn apply_locale_override_from_args<I, T>(args: I) -> Option<Locale>
where
    I: IntoIterator<Item = T>,
    T: Into<OsString>,
{
    let locale = locale_override_from_args(args);
    if let Some(locale) = locale {
        set_locale_override(locale);
    }
    locale
}

/// 从原始参数中读取语言覆盖但不修改进程状态。
///
/// 参数:
/// - `args`: 原始命令行参数
///
/// 返回:
/// - 识别到的语言；未提供或参数无效时返回空
fn locale_override_from_args<I, T>(args: I) -> Option<Locale>
where
    I: IntoIterator<Item = T>,
    T: Into<OsString>,
{
    let args = args.into_iter().map(Into::into).collect::<Vec<_>>();
    let locale = args.windows(2).find_map(|pair| {
        (pair[0] == "--lang")
            .then(|| pair[1].to_string_lossy())
            .and_then(|value| Locale::parse(&value))
    });
    let locale = locale.or_else(|| {
        args.iter().find_map(|arg| {
            arg.to_string_lossy()
                .strip_prefix("--lang=")
                .and_then(Locale::parse)
        })
    });
    locale
}

/// 设置当前进程的显式语言覆盖。
///
/// 参数:
/// - `locale`: 需要立即应用的语言
///
/// 返回:
/// - 无
fn set_locale_override(locale: Locale) {
    let value = match locale {
        Locale::En => LOCALE_EN_US,
        Locale::Zh => LOCALE_ZH_CN,
    };
    LOCALE_OVERRIDE.store(value, Ordering::Relaxed);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 验证常用中英文语言代码均可规范化。
    #[test]
    fn parses_supported_locale_codes() {
        assert_eq!(Locale::parse("en"), Some(Locale::En));
        assert_eq!(Locale::parse("en_US"), Some(Locale::En));
        assert_eq!(Locale::parse("zh-CN"), Some(Locale::Zh));
        assert_eq!(Locale::parse("zh_TW"), Some(Locale::Zh));
        assert_eq!(Locale::parse("ja-JP"), None);
    }

    /// 验证命令行语言参数支持分离形式和等号形式。
    #[test]
    fn reads_locale_override_from_cli_args() {
        assert_eq!(
            locale_override_from_args(["sai", "--lang", "zh-CN", "--help"]),
            Some(Locale::Zh)
        );
        assert_eq!(
            locale_override_from_args(["sai", "--lang=en-US", "--help"]),
            Some(Locale::En)
        );
    }

    /// 验证不受支持的高优先级区域值不会阻止后续中文候选生效。
    #[test]
    fn skips_unsupported_locale_candidates() {
        assert_eq!(
            detect_from_locale_values(["ja_JP.UTF-8", "zh_CN.UTF-8"]),
            Locale::Zh
        );
        assert_eq!(detect_from_locale_values(["C", "en_GB.UTF-8"]), Locale::En);
    }
}
