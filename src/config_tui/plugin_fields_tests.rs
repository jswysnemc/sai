use super::*;

/// 验证记忆表单完整保留近期新增字段。
#[test]
fn memory_fields_round_trip_recent_settings() {
    let mut config = AppConfig::default();
    config.plugins.memory.auto_skill_enabled = true;
    config.plugins.memory.snippet_chars = 321;
    config.plugins.memory.forget_after_days = 45;
    config.plugins.memory.learning_min_task_chars = 123;
    config.plugins.memory.learning_min_method_chars = 234;

    let fields = plugin_fields(&config, 10);
    let mut updated = AppConfig::default();
    apply_plugin_fields(&mut updated, 10, &fields).unwrap();

    assert_eq!(fields.len(), 17);
    assert!(updated.plugins.memory.auto_skill_enabled);
    assert_eq!(updated.plugins.memory.snippet_chars, 321);
    assert_eq!(updated.plugins.memory.forget_after_days, 45);
    assert_eq!(updated.plugins.memory.learning_min_task_chars, 123);
    assert_eq!(updated.plugins.memory.learning_min_method_chars, 234);
}

/// 验证知识库、视觉和诊断表单字段与写回索引一致。
#[test]
fn recent_plugin_fields_keep_complete_layouts() {
    let config = AppConfig::default();

    assert_eq!(plugin_fields(&config, 2).len(), 4);
    assert_eq!(plugin_fields(&config, 7).len(), 18);
    assert_eq!(plugin_fields(&config, 13).len(), 8);
    assert_eq!(plugin_fields(&config, 14).len(), 4);
}
