#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Locale {
    En,
    Zh,
}

impl Locale {
    pub fn detect() -> Self {
        for key in ["SAI_LANG", "LC_ALL", "LC_MESSAGES", "LANG"] {
            if let Ok(value) = std::env::var(key) {
                if let Some(locale) = Self::from_env_value(&value) {
                    return locale;
                }
            }
        }
        Self::En
    }

    fn from_env_value(value: &str) -> Option<Self> {
        let value = value.trim().to_ascii_lowercase();
        if value.is_empty() || value == "c" || value == "posix" {
            return None;
        }
        if value.starts_with("zh") {
            Some(Self::Zh)
        } else {
            Some(Self::En)
        }
    }
}

pub fn locale() -> Locale {
    Locale::detect()
}

pub fn is_zh() -> bool {
    locale() == Locale::Zh
}

pub fn text(en: &'static str, zh: &'static str) -> &'static str {
    if is_zh() {
        zh
    } else {
        en
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_chinese_locale_values() {
        assert_eq!(Locale::from_env_value("zh_CN.UTF-8"), Some(Locale::Zh));
        assert_eq!(Locale::from_env_value("zh_TW"), Some(Locale::Zh));
    }

    #[test]
    fn detects_english_locale_values() {
        assert_eq!(Locale::from_env_value("en_US.UTF-8"), Some(Locale::En));
        assert_eq!(Locale::from_env_value("ja_JP.UTF-8"), Some(Locale::En));
        assert_eq!(Locale::from_env_value("C"), None);
    }
}
