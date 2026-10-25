use super::TuiStyle;
use crate::config::DisplayConfig;

/// 【终端】【样式测试】验证旧配置默认值、序列化取值与非法配置拒绝。
/// 参数: 无；返回: 无。
#[test]
fn tui_style_config_is_backward_compatible_and_strict() {
    let old: DisplayConfig = serde_json::from_str("{}").unwrap();
    assert_eq!(old.tui_style, TuiStyle::Classic);
    assert!(old.fullscreen);
    for style in [TuiStyle::Classic, TuiStyle::Deepseek] {
        let config = DisplayConfig {
            tui_style: style,
            ..Default::default()
        };
        let value = serde_json::to_value(&config).unwrap();
        assert_eq!(value["tui_style"], style.as_str());
        let restored: DisplayConfig = serde_json::from_value(value).unwrap();
        assert_eq!(restored.tui_style, style);
    }
    assert!(serde_json::from_str::<DisplayConfig>(r#"{"tui_style":"unknown"}"#).is_err());
}

/// 【终端】【样式测试】验证名称解析与双向切换。
/// 参数: 无；返回: 无。
#[test]
fn tui_style_names_parse_and_toggle() {
    assert_eq!(TuiStyle::parse(" DeepSeek "), Some(TuiStyle::Deepseek));
    assert_eq!(TuiStyle::parse("classic"), Some(TuiStyle::Classic));
    assert_eq!(TuiStyle::parse("other"), None);
    assert_eq!(TuiStyle::Classic.toggled(), TuiStyle::Deepseek);
    assert_eq!(TuiStyle::Deepseek.toggled(), TuiStyle::Classic);
}
