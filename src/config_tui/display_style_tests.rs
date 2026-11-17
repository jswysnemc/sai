use super::*;

/// 【终端】【样式测试】显示表单验证样式选项，非法值不能部分更新配置。
/// 参数: 无；返回: 无。
#[test]
fn tui_mode_form_validates_before_updating_configuration() {
    let values = [
        "summary",
        "full",
        "true",
        "true",
        "true",
        "1000",
        "false",
        "false",
        "false",
        "head",
        "4",
        "2",
        "separator",
    ];
    let mut fields = values
        .iter()
        .map(|value| Field::new("test", value.to_string()))
        .collect::<Vec<_>>();
    let mut config = AppConfig::default();
    apply_display_fields(&mut config, &fields).unwrap();
    assert!(!config.display.fullscreen);
    assert_eq!(
        config.display.input_style,
        crate::config::InputStyle::Separator
    );
    let saved = serde_json::to_value(&config).unwrap();
    fields[6].value = "unknown".to_string();
    fields[0].value = "hidden".to_string();
    assert!(apply_display_fields(&mut config, &fields).is_err());
    assert_eq!(serde_json::to_value(&config).unwrap(), saved);
    fields[6].value = "false".to_string();
    fields[12].value = "invalid".to_string();
    assert!(apply_display_fields(&mut config, &fields).is_err());
    assert_eq!(serde_json::to_value(&config).unwrap(), saved);
}

/// 【终端】【语言实时切换测试】验证移除 OnceLock 后 rtk 过滤标签随当前语言实时变化。
#[test]
fn rtk_denylist_label_switches_dynamically_with_locale() {
    let en = crate::i18n::with_locale(crate::i18n::Locale::En, rtk_denylist_label);
    let zh = crate::i18n::with_locale(crate::i18n::Locale::Zh, rtk_denylist_label);
    assert!(en.contains("Commands kept out of rtk"));
    assert!(zh.contains("不走 rtk 的命令"));
}
