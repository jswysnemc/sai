use super::*;

/// 【上下文】【模型上限测试】验证设置、刷新容量和恢复最大不会覆盖 API 元数据。
/// 参数: 无；返回: 无
#[test]
fn limit_preserves_capacity_and_maximum_tracks_metadata_updates() {
    let mut config = AppConfig::default();
    let model = config.providers[0].default_model.clone();
    config.providers[0].set_model_context_chars_for(&model, Some(1_000_000));
    config.providers[0].set_model_context_limit_tokens_for(&model, Some(300_000));
    assert_eq!(config.active_context_window_tokens().unwrap(), 300_000);
    assert_eq!(
        config.providers[0].model_context_chars_for(&model),
        Some(1_000_000)
    );

    config.providers[0].set_model_context_chars_for(&model, Some(800_000));
    assert_eq!(config.active_context_window_tokens().unwrap(), 300_000);
    config.providers[0].set_model_context_limit_tokens_for(&model, None);
    assert_eq!(config.active_context_window_tokens().unwrap(), 800_000);
    assert_eq!(
        config.providers[0].model_context_chars_for(&model),
        Some(800_000)
    );
}

/// 【上下文】【模型上限测试】验证已知容量限制和未知容量的手动预算。
/// 参数: 无；返回: 无
#[test]
fn limit_respects_known_capacity_and_overrides_unknown_fallback() {
    let mut provider = ProviderConfig::new_openai_compatible();
    provider.set_model_context_limit_tokens_for("custom", Some(500_000));
    assert_eq!(
        provider.effective_context_tokens_for("custom", 128_000),
        500_000
    );
    provider
        .model_context_chars
        .insert("custom".to_string(), 64_000);
    assert_eq!(
        provider.effective_context_tokens_for("custom", 128_000),
        64_000
    );
    provider.set_model_context_limit_tokens_for("custom", None);
    assert_eq!(provider.model_context_chars_for("custom"), Some(64_000));
    assert!(!provider.model_metadata.contains_key("custom"));
}

/// 【上下文】【模型上限测试】验证供应商与模型之间相互隔离，序列化保留独立上限。
/// 参数: 无；返回: 无
#[test]
fn limits_round_trip_per_provider_and_model() {
    let mut config = AppConfig::default();
    let model = config.providers[0].default_model.clone();
    config.providers[0].set_model_context_chars_for(&model, Some(1_000_000));
    config.providers[0].set_model_context_limit_tokens_for(&model, Some(200_000));
    config.providers[0].set_model_context_limit_tokens_for("other-model", Some(400_000));
    let serialized = serde_json::to_string(&config).unwrap();
    let restored: AppConfig = serde_json::from_str(&serialized).unwrap();
    assert_eq!(restored.active_context_window_tokens().unwrap(), 200_000);
    assert_eq!(
        restored.providers[0].model_context_chars_for(&model),
        Some(1_000_000)
    );
    assert_eq!(
        restored.providers[0].model_context_limit_tokens_for("other-model"),
        Some(400_000)
    );
    assert_eq!(
        restored.providers[1].model_context_limit_tokens_for(&model),
        None
    );
}

/// 【上下文】【模型上限测试】验证旧配置兼容、单位解析和空元数据清理。
/// 参数: 无；返回: 无
#[test]
fn metadata_accepts_units_and_retains_limit_only_entries() {
    let old: ModelMetadata = serde_json::from_str(r#"{"context_chars":"1m"}"#).unwrap();
    assert_eq!(old.context_limit_tokens, None);
    let metadata: ModelMetadata =
        serde_json::from_str(r#"{"context_limit_tokens":"350k"}"#).unwrap();
    assert_eq!(metadata.context_limit_tokens, Some(350_000));
    assert!(!metadata.is_empty());
    let mut provider = ProviderConfig::new_openai_compatible();
    provider
        .model_metadata
        .insert("custom".to_string(), metadata);
    provider.set_model_context_chars_for("custom", None);
    assert_eq!(
        provider.model_context_limit_tokens_for("custom"),
        Some(350_000)
    );
}

/// 【上下文】【模型上限测试】验证真实配置保存和重新加载保留上限及 API 容量。
/// 参数: 无；返回: 无
#[test]
fn saved_limit_survives_config_reload_and_maximum_reset() {
    let directory = tempfile::tempdir().unwrap();
    let paths = crate::paths::SaiPaths::for_tests(directory.path());
    let mut config = AppConfig::default();
    let model = config.providers[0].default_model.clone();
    config.providers[0].set_model_context_chars_for(&model, Some(1_000_000));
    config.providers[0].set_model_context_limit_tokens_for(&model, Some(500_000));
    config.save(&paths).unwrap();
    let mut restored = AppConfig::load(&paths).unwrap();
    assert_eq!(restored.active_context_window_tokens().unwrap(), 500_000);
    assert_eq!(
        restored.providers[0].model_context_chars_for(&model),
        Some(1_000_000)
    );
    restored.providers[0].set_model_context_limit_tokens_for(&model, None);
    restored.save(&paths).unwrap();
    let restored = AppConfig::load(&paths).unwrap();
    assert_eq!(restored.active_context_window_tokens().unwrap(), 1_000_000);
    assert!(!std::fs::read_to_string(&paths.config_file)
        .unwrap()
        .contains("context_limit_tokens"));
}
