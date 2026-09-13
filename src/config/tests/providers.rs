use super::*;

#[test]
fn provider_config_can_be_saved_without_active_model() {
    let mut config = AppConfig::default();
    config.providers[0].models.clear();
    config.providers[0].default_model.clear();
    assert!(config.validate().is_ok());
}

#[test]
fn provider_model_choices_ignore_unconfigured_models() {
    let mut config = AppConfig::default();
    let provider_id = config.providers[0].id.clone();
    config.providers[0].models.clear();
    config.providers[0].default_model.clear();
    assert!(!config
        .provider_model_choices()
        .iter()
        .any(|choice| choice.provider_id == provider_id));
}

/// 停用的供应商不出现在模型选择列表里。
///
/// 允许它出现就等于允许被选中并发请求，停用开关形同虚设。
#[test]
fn disabled_providers_are_excluded_from_model_choices() {
    let mut config = AppConfig::default();
    let provider_id = config.providers[0].id.clone();
    config.providers[0].models = vec!["model-a".to_string()];
    assert!(config
        .provider_model_choices()
        .iter()
        .any(|choice| choice.provider_id == provider_id));

    config.providers[0].enabled = false;

    assert!(!config
        .provider_model_choices()
        .iter()
        .any(|choice| choice.provider_id == provider_id));
}

/// 只改 `default_model` 而未加进 `models` 时，新模型仍要可选。
///
/// `/config` 的编辑路径与直接手改配置文件都可能让 `default_model` 落在
/// `models` 之外；只读 `models` 会让当前生效的模型在 `/model` 里凭空消失。
#[test]
fn default_model_outside_models_list_is_still_selectable() {
    let mut config = AppConfig::default();
    let provider_id = config.providers[0].id.clone();
    config.providers[0].models = vec!["old-model".to_string()];
    config.providers[0].default_model = "new-model".to_string();

    let models = models_for(&config, &provider_id);

    assert!(
        models.iter().any(|model| model == "old-model"),
        "{models:?}"
    );
    assert!(
        models.iter().any(|model| model == "new-model"),
        "{models:?}"
    );
}

/// `default_model` 已在 `models` 里时不重复出现。
#[test]
fn default_model_inside_models_list_is_not_duplicated() {
    let mut config = AppConfig::default();
    let provider_id = config.providers[0].id.clone();
    config.providers[0].models = vec!["model-a".to_string(), "model-b".to_string()];
    config.providers[0].default_model = "model-a".to_string();

    assert_eq!(
        models_for(&config, &provider_id),
        vec!["model-a".to_string(), "model-b".to_string()]
    );
}

/// `models` 为空时回落到 `default_model`，保持既有行为。
#[test]
fn empty_models_list_falls_back_to_default_model() {
    let mut config = AppConfig::default();
    let provider_id = config.providers[0].id.clone();
    config.providers[0].models.clear();
    config.providers[0].default_model = "only-model".to_string();

    assert_eq!(
        models_for(&config, &provider_id),
        vec!["only-model".to_string()]
    );
}

/// 取出指定供应商在模型选择列表里的模型标识。
fn models_for(config: &AppConfig, provider_id: &str) -> Vec<String> {
    config
        .provider_model_choices()
        .into_iter()
        .filter(|choice| choice.provider_id == provider_id)
        .map(|choice| choice.model)
        .collect()
}

/// 停用的供应商不能被解析，避免绕过开关继续发请求。
#[test]
fn disabled_provider_cannot_be_resolved() {
    let mut config = AppConfig::default();
    let provider_id = config.providers[0].id.clone();
    assert!(config.provider(Some(&provider_id)).is_ok());

    config.providers[0].enabled = false;

    let error = config
        .provider(Some(&provider_id))
        .expect_err("停用的供应商必须解析失败");
    assert!(error.to_string().contains("disabled"));
}

/// TUI 把无 default_model 的供应商设为当前项时，不得回退到已停用的 opencode。
#[test]
fn normalize_keeps_enabled_provider_instead_of_disabled_opencode() {
    let mut config = AppConfig::default();
    config
        .providers
        .iter_mut()
        .find(|provider| provider.id == crate::default_models::OPENCODE_PROVIDER_ID)
        .unwrap()
        .enabled = false;

    let mut custom = ProviderConfig::new_openai_compatible();
    custom.id = "b.ai".to_string();
    custom.display_name = "b.ai".to_string();
    custom.base_url = "https://api.b.ai/v1".to_string();
    custom.model_metadata.insert(
        "deepseek-v4-flash".to_string(),
        ModelMetadata {
            context_chars: Some(70_000),
            ..ModelMetadata::default()
        },
    );
    config.providers.push(custom);
    config.active_provider = "b.ai".to_string();

    config.normalize_builtin_providers();

    assert_eq!(config.active_provider, "b.ai");
    assert_eq!(
        config.provider(None).unwrap().default_model,
        "deepseek-v4-flash"
    );
    assert!(config.validate().is_ok());
}

/// 当前供应商已停用时，回退到其他已启用且带模型的供应商。
#[test]
fn normalize_falls_back_to_enabled_provider_when_active_is_disabled() {
    let mut config = AppConfig::default();
    let disabled_id = config.providers[0].id.clone();
    config.providers[0].enabled = false;
    config.active_provider = disabled_id.clone();
    config.providers[1].models = vec!["usable-model".to_string()];
    config.providers[1].default_model = "usable-model".to_string();

    config.normalize_builtin_providers();

    assert_ne!(config.active_provider, disabled_id);
    assert!(config.provider(None).is_ok());
    assert!(config.validate().is_ok());
}

/// 写入已停用的供应商不得把它提升为当前项。
#[test]
fn upsert_does_not_activate_disabled_provider() {
    let mut config = AppConfig::default();
    let active = config.active_provider.clone();
    let mut provider = ProviderConfig::new_openai_compatible();
    provider.id = "disabled-new".to_string();
    provider.enabled = false;

    config.upsert_provider(provider);

    assert_eq!(config.active_provider, active);
}

/// 已停用的供应商不能被设为当前模型。
#[test]
fn set_active_provider_model_rejects_disabled_provider() {
    let mut config = AppConfig::default();
    let provider_id = config.providers[0].id.clone();
    config.providers[0].enabled = false;

    let error = config
        .set_active_provider_model(&provider_id, "any-model")
        .expect_err("停用的供应商不能被激活");
    assert!(error.to_string().contains("disabled"));
}

/// 未写入配置文件的供应商默认处于启用状态。
#[test]
fn providers_default_to_enabled_when_the_field_is_absent() {
    let raw = serde_json::json!({
        "id": "p",
        "display_name": "p",
        "base_url": "https://example.test/v1"
    });

    let provider: ProviderConfig = serde_json::from_value(raw).unwrap();

    assert!(provider.enabled);
}

#[test]
fn new_openai_compatible_provider_has_no_active_model() {
    let provider = ProviderConfig::new_openai_compatible();

    assert!(provider.models.is_empty());
    assert!(provider.default_model.is_empty());
}

#[test]
fn default_templates_include_official_anthropic_provider() {
    let provider = ProviderConfig::default_templates()
        .into_iter()
        .find(|provider| provider.id == "anthropic")
        .unwrap();

    assert_eq!(provider.protocol, "anthropic");
    assert_eq!(provider.base_url, "https://api.anthropic.com/v1");
    assert!(provider.default_model.starts_with("claude-"));
}

#[test]
fn official_anthropic_uses_family_context_fallback() {
    let mut config = AppConfig::default();
    config.active_provider = "anthropic".to_string();

    assert_eq!(config.active_context_window_tokens().unwrap(), 200_000);
}

#[test]
fn explicit_anthropic_context_overrides_family_fallback() {
    let mut config = AppConfig::default();
    let provider = config
        .providers
        .iter_mut()
        .find(|provider| provider.id == "anthropic")
        .unwrap();
    let model = provider.default_model.clone();
    provider.set_model_context_chars_for(&model, Some(160_000));
    config.active_provider = "anthropic".to_string();

    assert_eq!(config.active_context_window_tokens().unwrap(), 160_000);
}

#[test]
fn remove_active_provider_model_clears_removed_current_model() {
    let mut config = AppConfig::default();
    let provider_id = config.providers[0].id.clone();
    config.providers[0].models = vec!["old-model".to_string(), "next-model".to_string()];
    config.providers[0].default_model = "old-model".to_string();
    config.session.new_session_provider_id = provider_id.clone();
    config.session.new_session_model = "old-model".to_string();
    config.providers[0]
        .model_context_chars
        .insert("old-model".to_string(), 8192);
    config.providers[0].model_metadata.insert(
        "old-model".to_string(),
        ModelMetadata {
            context_chars: Some(8192),
            max_output_tokens: None,
            tags: vec!["web_search".to_string()],
            tools_enabled: None,
            web_search_tool_mode: None,
            thinking_levels: Vec::new(),
            deepseek_anchor_mode: None,
        },
    );

    config
        .remove_active_provider_model(&provider_id, "old-model")
        .unwrap();

    assert_eq!(config.providers[0].models, vec!["next-model"]);
    assert_eq!(config.providers[0].default_model, "next-model");
    assert!(!config.providers[0]
        .model_context_chars
        .contains_key("old-model"));
    assert!(!config.providers[0].model_metadata.contains_key("old-model"));
    assert!(config.session.new_session_provider_id.is_empty());
    assert!(config.session.new_session_model.is_empty());
}

#[test]
fn remove_active_provider_model_clears_last_current_model() {
    let mut config = AppConfig::default();
    let provider_id = config.providers[0].id.clone();
    config.providers[0].models = vec!["old-model".to_string()];
    config.providers[0].default_model = "old-model".to_string();

    config
        .remove_active_provider_model(&provider_id, "old-model")
        .unwrap();

    assert!(config.providers[0].models.is_empty());
    assert!(config.providers[0].default_model.is_empty());
    assert!(!config
        .provider_model_choices()
        .iter()
        .any(|choice| choice.provider_id == provider_id));
}

#[test]
fn remove_provider_clears_all_associated_model_references() {
    let mut config = AppConfig::default();
    let removed_id = config.providers[0].id.clone();
    config.active_provider = removed_id.clone();
    config.plugins.vision.vision_provider_id = removed_id.clone();
    config.plugins.vision.vision_model = "vision-model".to_string();
    config.subagent.provider_id = removed_id.clone();
    config.subagent.model = "subagent-model".to_string();
    config.context.compaction_provider_id = removed_id.clone();
    config.context.compaction_model = "compaction-model".to_string();
    config.session.new_session_provider_id = removed_id.clone();
    config.session.new_session_model = config.providers[0].default_model.clone();

    let removed = config.remove_provider(&removed_id).unwrap();

    assert_eq!(removed.id, removed_id);
    assert_ne!(config.active_provider, removed_id);
    assert!(config.plugins.vision.vision_provider_id.is_empty());
    assert!(config.plugins.vision.vision_model.is_empty());
    assert!(config.subagent.provider_id.is_empty());
    assert!(config.subagent.model.is_empty());
    assert!(config.context.compaction_provider_id.is_empty());
    assert!(config.context.compaction_model.is_empty());
    assert!(config.session.new_session_provider_id.is_empty());
    assert!(config.session.new_session_model.is_empty());
    assert!(config.validate().is_ok());
}

#[test]
fn remove_provider_preserves_unrelated_model_references() {
    let mut config = AppConfig::default();
    let removed_id = config.providers[0].id.clone();
    let retained_id = config.providers[1].id.clone();
    config.plugins.vision.vision_provider_id = retained_id.clone();
    config.plugins.vision.vision_model = "vision-model".to_string();

    config.remove_provider(&removed_id).unwrap();

    assert_eq!(config.plugins.vision.vision_provider_id, retained_id);
    assert_eq!(config.plugins.vision.vision_model, "vision-model");
}

#[test]
fn remove_provider_rejects_deleting_last_provider() {
    let mut config = AppConfig::default();
    config.providers.truncate(1);
    let provider_id = config.providers[0].id.clone();

    assert!(config.remove_provider(&provider_id).is_err());
    assert_eq!(config.providers.len(), 1);
}

#[test]
fn active_context_window_tokens_prefers_model_metadata() {
    let mut config = AppConfig::default();
    let model = config.providers[0].default_model.clone();
    config.providers[0]
        .model_context_chars
        .insert(model.clone(), 32_000);
    config.providers[0].model_metadata.insert(
        model,
        ModelMetadata {
            context_chars: Some(128_000),
            max_output_tokens: None,
            tools_enabled: None,
            tags: Vec::new(),
            web_search_tool_mode: None,
            thinking_levels: Vec::new(),
            deepseek_anchor_mode: None,
        },
    );

    assert_eq!(config.active_context_window_tokens().unwrap(), 128_000);
}

/// 验证未指定压缩模型时沿用当前会话模型。
#[test]
fn compaction_runtime_config_inherits_active_model() {
    let mut config = AppConfig::default();
    let provider_id = config.providers[2].id.clone();
    let model = config.providers[2].default_model.clone();
    config
        .set_active_provider_model(&provider_id, &model)
        .unwrap();

    let resolved = config.compaction_runtime_config().unwrap();

    assert_eq!(resolved.active_provider, provider_id);
    assert_eq!(resolved.provider(None).unwrap().default_model, model);
}

/// 验证显式压缩模型覆盖会话模型，但不修改原配置中的会话选择。
#[test]
fn compaction_runtime_config_uses_dedicated_model() {
    let mut config = AppConfig::default();
    let conversation_provider = config.active_provider.clone();
    let dedicated_provider = config.providers[2].id.clone();
    let dedicated_model = config.providers[2].default_model.clone();
    config.context.compaction_provider_id = dedicated_provider.clone();
    config.context.compaction_model = dedicated_model.clone();

    let resolved = config.compaction_runtime_config().unwrap();

    assert_eq!(resolved.active_provider, dedicated_provider);
    assert_eq!(
        resolved.provider(None).unwrap().default_model,
        dedicated_model
    );
    assert_eq!(config.active_provider, conversation_provider);
}

#[test]
fn model_metadata_context_accepts_unit_strings() {
    let metadata: ModelMetadata = serde_json::from_str(r#"{"context_chars":"128k"}"#).unwrap();

    assert_eq!(metadata.context_chars, Some(128_000));
}

#[test]
fn provider_validation_rejects_invalid_model_tag() {
    let mut config = AppConfig::default();
    let model = config.providers[0].default_model.clone();
    config.providers[0].set_model_tags_for(&model, vec!["unknown".to_string()]);

    let err = config.validate().unwrap_err();

    assert!(err.to_string().contains("model_metadata tag"));
}

#[test]
fn provider_validation_rejects_duplicate_ids() {
    let mut config = AppConfig::default();
    let duplicate = config.providers[0].clone();
    config.providers.push(duplicate);

    let error = config.validate().unwrap_err();

    assert!(error.to_string().contains("duplicate provider id"));
}

#[test]
fn active_model_tools_default_to_enabled() {
    let config = AppConfig::default();

    assert!(config.active_model_tools_enabled().unwrap());
}

#[test]
fn active_model_tools_can_be_disabled() {
    let mut config = AppConfig::default();
    let model = config.providers[0].default_model.clone();
    config.providers[0].set_model_tools_enabled_for(&model, false);

    assert!(!config.active_model_tools_enabled().unwrap());
}

#[test]
fn deepseek_anchor_requires_explicit_model_switch_even_for_proxy_names() {
    let mut config = AppConfig::default();
    {
        let provider = &mut config.providers[0];
        provider.id = "muyuan".to_string();
        provider.base_url = "https://proxy.example/v1".to_string();
        provider.default_model = "reasoning-model".to_string();
        provider.models = vec![provider.default_model.clone()];
    }
    config.active_provider = "muyuan".to_string();

    assert!(!config.active_deepseek_anchor_enabled().unwrap());
    let model = config.providers[0].default_model.clone();
    config.providers[0].set_model_deepseek_anchor_mode_for(&model, DEEPSEEK_ANCHOR_MODE_STANDARD);
    assert!(config.active_deepseek_anchor_enabled().unwrap());
}

#[test]
fn selects_provider_model_by_web_search_tag() {
    let mut config = AppConfig::default();
    let mut provider = ProviderConfig::new_openai_compatible();
    provider.id = "web".to_string();
    provider.display_name = "Web".to_string();
    provider.base_url = "https://example.invalid/v1".to_string();
    provider.models.push("web-model".to_string());
    provider.default_model = "web-model".to_string();
    provider.set_model_tags_for("web-model", vec![MODEL_TAG_WEB_SEARCH.to_string()]);
    config.providers.push(provider);

    let choice = config
        .select_active_provider_model_with_tag(MODEL_TAG_WEB_SEARCH)
        .unwrap();

    assert_eq!(choice.provider_id, "web");
    assert_eq!(choice.model, "web-model");
    assert_eq!(config.active_provider, "web");
    assert_eq!(config.provider(None).unwrap().default_model, "web-model");
}

#[test]
fn provider_validation_rejects_zero_legacy_context() {
    let mut config = AppConfig::default();
    let model = config.providers[0].default_model.clone();
    config.providers[0].model_context_chars.insert(model, 0);

    let err = config.validate().unwrap_err();

    assert!(err.to_string().contains("model_context_chars"));
}
