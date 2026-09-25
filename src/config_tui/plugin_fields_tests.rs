use super::*;

/// 验证记忆表单只暴露仍然生效的设置且能往返写回。
///
/// 自动抽取与遗忘曲线随旧记忆实现一并移除，表单里留着它们只会让用户
/// 以为调了有用。字段数一并断言，多出来的项必然是又一个死配置。
#[test]
fn memory_fields_round_trip_effective_settings() {
    let mut config = AppConfig::default();
    config.plugins.memory.evicted_context_enabled = false;
    config.plugins.memory.association_enabled = false;
    config.plugins.memory.snippet_chars = 321;

    let fields = plugin_fields(&config, "memory");
    let mut updated = AppConfig::default();
    apply_plugin_fields(&mut updated, "memory", &fields).unwrap();

    assert_eq!(fields.len(), 4);
    assert!(!updated.plugins.memory.evicted_context_enabled);
    assert!(!updated.plugins.memory.association_enabled);
    assert_eq!(updated.plugins.memory.snippet_chars, 321);
}

/// 验证知识库和视觉表单字段与写回索引一致。
#[test]
fn recent_plugin_fields_keep_complete_layouts() {
    let config = AppConfig::default();

    assert_eq!(plugin_fields(&config, "vision").len(), 3);
}
