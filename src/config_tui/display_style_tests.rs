use super::*;

/// 【终端】【样式测试】显示表单验证样式选项，非法值不能部分更新配置。
/// 参数: 无；返回: 无。
#[test]
fn tui_mode_form_validates_before_updating_configuration() {
    let values = [
        "summary", "full", "true", "true", "true", "1000", "false", "false", "false", "head", "4",
        "2",
    ];
    let mut fields = values
        .iter()
        .map(|value| Field::new("test", value.to_string()))
        .collect::<Vec<_>>();
    let mut config = AppConfig::default();
    apply_display_fields(&mut config, &fields).unwrap();
    assert!(!config.display.fullscreen);
    let saved = serde_json::to_value(&config).unwrap();
    fields[6].value = "unknown".to_string();
    fields[0].value = "hidden".to_string();
    assert!(apply_display_fields(&mut config, &fields).is_err());
    assert_eq!(serde_json::to_value(&config).unwrap(), saved);
}
