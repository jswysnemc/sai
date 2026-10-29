use super::TuiMode;
use crate::config::DisplayConfig;

/// 【终端】【布局配置】默认全屏，旧 deepseek 样式迁移为全屏，旧样式不再写入配置。
/// 参数: 无；返回: 无。
#[test]
fn tui_mode_config_is_backward_compatible_and_strict() {
    let default: DisplayConfig = serde_json::from_str("{}").unwrap();
    assert!(default.fullscreen);
    assert_eq!(TuiMode::default(), TuiMode::Fullscreen);
    for (legacy, fullscreen) in [("classic", false), ("deepseek", true)] {
        let config: DisplayConfig =
            serde_json::from_value(serde_json::json!({"tui_style":legacy,"fullscreen":false}))
                .unwrap();
        assert_eq!(config.fullscreen, fullscreen);
        let serialized = serde_json::to_value(config).unwrap();
        assert!(serialized.get("tui_style").is_none());
        assert!(serialized.get("tui_mode").is_none());
    }
    assert!(serde_json::from_str::<DisplayConfig>(r#"{"tui_style":"unknown"}"#).is_err());
}

/// 【终端】【布局切换】全屏与内联使用明确名称，旧风格名称不出现在界面命令中。
/// 参数: 无；返回: 无。
#[test]
fn tui_mode_names_parse_and_toggle() {
    assert_eq!(TuiMode::parse(" Fullscreen "), Some(TuiMode::Fullscreen));
    assert_eq!(TuiMode::parse("inline"), Some(TuiMode::Inline));
    assert_eq!(TuiMode::parse("other"), None);
    assert_eq!(TuiMode::Inline.toggled(), TuiMode::Fullscreen);
    assert_eq!(TuiMode::Fullscreen.toggled(), TuiMode::Inline);
}
