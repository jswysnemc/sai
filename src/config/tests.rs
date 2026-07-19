use super::*;

#[test]
fn provider_config_can_be_saved_without_active_model() {
    let mut config = AppConfig::default();
    config.providers[0].models.clear();
    config.providers[0].default_model.clear();
    assert!(config.validate().is_ok());
}

/// 验证旧版应用配置会补齐终端权限默认值。
///
/// 参数:
/// - 无
///
/// 返回:
/// - 无
#[test]
fn legacy_app_config_defaults_terminal_permission_mode_to_yolo() {
    let mut value = serde_json::to_value(AppConfig::default()).unwrap();
    value.as_object_mut().unwrap().remove("permission");

    let config: AppConfig = serde_json::from_value(value).unwrap();

    assert_eq!(config.permission.default_mode, DefaultPermissionMode::Yolo);
    assert_eq!(config.permission.tui_mode(), DefaultPermissionMode::Yolo);
    assert_eq!(config.permission.cli_mode(), DefaultPermissionMode::Yolo);
}

/// 验证旧版应用配置会补齐网页终端配置。
///
/// 参数:
/// - 无
///
/// 返回:
/// - 无
#[test]
fn legacy_app_config_defaults_web_terminal_shell() {
    let mut value = serde_json::to_value(AppConfig::default()).unwrap();
    value.as_object_mut().unwrap().remove("terminal");

    let config: AppConfig = serde_json::from_value(value).unwrap();

    assert_eq!(config.terminal.shell, TerminalConfig::default().shell);
}

/// 验证网页终端 Shell 会保留用户配置值。
#[test]
fn web_terminal_shell_preserves_user_configuration() {
    let mut config = AppConfig::default();
    config.terminal.shell = "custom-shell".to_string();

    let restored: AppConfig =
        serde_json::from_value(serde_json::to_value(config).unwrap()).unwrap();

    assert_eq!(restored.terminal.shell, "custom-shell");
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
    config.plugins.knowledge_base.embedding_provider_id = removed_id.clone();
    config.plugins.knowledge_base.embedding_model = "embedding-model".to_string();
    config.subagent.provider_id = removed_id.clone();
    config.subagent.model = "subagent-model".to_string();
    config.context.compaction_provider_id = removed_id.clone();
    config.context.compaction_model = "compaction-model".to_string();

    let removed = config.remove_provider(&removed_id).unwrap();

    assert_eq!(removed.id, removed_id);
    assert_ne!(config.active_provider, removed_id);
    assert!(config.plugins.vision.vision_provider_id.is_empty());
    assert!(config.plugins.vision.vision_model.is_empty());
    assert!(config
        .plugins
        .knowledge_base
        .embedding_provider_id
        .is_empty());
    assert!(config.plugins.knowledge_base.embedding_model.is_empty());
    assert!(config.subagent.provider_id.is_empty());
    assert!(config.subagent.model.is_empty());
    assert!(config.context.compaction_provider_id.is_empty());
    assert!(config.context.compaction_model.is_empty());
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
fn validate_rejects_invalid_temperature_and_timeout() {
    let mut config = AppConfig::default();
    config.providers[0].temperature = 3.0;
    assert!(config.validate().is_err());
    config.providers[0].temperature = 0.7;
    config.providers[0].timeout_seconds = 0;
    assert!(config.validate().is_err());
    config.providers[0].timeout_seconds = 60;
    config.providers[0].anthropic_max_tokens = 0;
    assert!(config.validate().is_err());
}

#[test]
fn display_readable_tool_names_defaults_enabled() {
    let display: DisplayConfig = serde_json::from_str(r#"{"tool_calls":"summary"}"#).unwrap();
    assert!(display.readable_tool_names);
}

#[test]
fn display_wait_detail_options_default_enabled() {
    let display: DisplayConfig = serde_json::from_str(r#"{"tool_calls":"summary"}"#).unwrap();
    assert!(display.wait_show_model);
    assert!(display.wait_show_thinking_level);
}

#[test]
fn display_wait_detail_options_can_be_disabled() {
    let display: DisplayConfig =
        serde_json::from_str(r#"{"wait_show_model":false,"wait_show_thinking_level":false}"#)
            .unwrap();
    assert!(!display.wait_show_model);
    assert!(!display.wait_show_thinking_level);
}

#[test]
fn display_repl_transcript_row_cap_defaults_to_bounded_value() {
    let display: DisplayConfig = serde_json::from_str(r#"{"tool_calls":"summary"}"#).unwrap();

    assert_eq!(display.repl_transcript_row_cap, 5_000);
}

#[test]
fn progressive_tool_loading_defaults_disabled() {
    let config = AppConfig::default();
    assert!(!config.tools.progressive_loading_enabled);
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

#[test]
fn background_command_defaults_are_enabled() {
    let config = AppConfig::default();
    assert!(config.tools.background_commands_enabled);
    assert_eq!(config.tools.background_command_timeout_seconds, 0);
    assert!(config.tools.background_command_log_max_bytes > 0);
    assert!(config.tools.background_command_stop_grace_seconds > 0);
}

#[test]
fn gateway_defaults_are_valid() {
    let config = AppConfig::default();

    assert!(config.validate().is_ok());
    assert!(!config.gateways.qq.enabled);
    assert!(!config.gateways.weixin.enabled);
    assert_eq!(config.gateways.qq.transport, "websocket");
    assert_eq!(config.gateways.qq.listen, "127.0.0.1:8766");
    assert_eq!(config.gateways.qq.base_url, "https://api.sgroup.qq.com");
    assert_eq!(
        config.gateways.weixin.base_url,
        "https://ilinkai.weixin.qq.com"
    );
    assert_eq!(
        config.gateways.weixin.cdn_base_url,
        "https://novac2c.cdn.weixin.qq.com/c2c"
    );
    assert_eq!(config.gateways.weixin.bot_type, "3");
}

#[test]
fn gateway_validation_rejects_invalid_qq_transport() {
    let mut config = AppConfig::default();
    config.gateways.qq.transport = "polling".to_string();

    let err = config.validate().unwrap_err();

    assert!(err.to_string().contains("gateways.qq.transport"));
}

#[test]
fn gateway_validation_rejects_invalid_listen_address() {
    let mut config = AppConfig::default();
    config.gateways.qq.listen = "not-a-socket".to_string();

    let err = config.validate().unwrap_err();

    assert!(err.to_string().contains("gateways.qq.listen"));
}

#[test]
fn gateway_validation_rejects_invalid_qq_token() {
    let mut config = AppConfig::default();
    config.gateways.qq.token = "missing-secret".to_string();

    let err = config.validate().unwrap_err();

    assert!(err.to_string().contains("gateways.qq.token"));
}

#[test]
fn meme_library_defaults_follow_persona() {
    let memes = MemesPluginConfig::default();
    assert_eq!(memes.library_for_persona(""), "sai");
    assert_eq!(
        memes.library_for_persona("Custom Persona"),
        "custom-persona"
    );
    assert!(memes.auto_send_enabled);
    assert_eq!(memes.auto_send_probability, 0.2);
    assert_eq!(memes.auto_send_min_confidence, 0.8);
}
