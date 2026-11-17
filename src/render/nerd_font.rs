//! Nerd Fonts 私有区图标开关的统一判定。

/// 【终端图标】【字体开关】读取环境变量判定是否启用 Nerd Fonts 私有区图标。
/// 参数: 无；返回: 默认返回 true，当 SAI_NERD_FONT 显式设为 0、false、off 或 no 时返回 false。
pub(crate) fn nerd_font_enabled() -> bool {
    parse_nerd_font_flag(std::env::var("SAI_NERD_FONT").ok().as_deref())
}

/// 【终端图标】【配置解析】解析字体开关文本，未配置或空白时默认启用。
/// 参数: raw 为可选环境变量文本；返回: 是否启用私有区图标。
pub(crate) fn parse_nerd_font_flag(raw: Option<&str>) -> bool {
    let Some(value) = raw.map(str::trim).filter(|value| !value.is_empty()) else {
        return true;
    };
    !matches!(
        value.to_ascii_lowercase().as_str(),
        "0" | "false" | "off" | "no"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 【终端图标测试】【开关解析】验证默认开启及常见关闭字面量识别。
    /// 参数: 无；返回: 无。
    #[test]
    fn parses_nerd_font_env_values() {
        assert!(parse_nerd_font_flag(None));
        assert!(parse_nerd_font_flag(Some("")));
        assert!(parse_nerd_font_flag(Some("1")));
        assert!(parse_nerd_font_flag(Some("true")));
        assert!(!parse_nerd_font_flag(Some("0")));
        assert!(!parse_nerd_font_flag(Some("false")));
        assert!(!parse_nerd_font_flag(Some("OFF")));
        assert!(!parse_nerd_font_flag(Some("no")));
    }
}
