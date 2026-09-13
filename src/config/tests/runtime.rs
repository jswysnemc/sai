use super::*;

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

/// 【会话】【配置兼容】验证旧版配置会补齐新会话模型与思考等级默认值。
///
/// 参数:
/// - 无
///
/// 返回:
/// - 无
#[test]
fn legacy_session_config_defaults_new_session_preferences() {
    let session: SessionConfig = serde_json::from_str(r#"{"auto_title_enabled":false}"#).unwrap();

    assert!(session.new_session_provider_id.is_empty());
    assert!(session.new_session_model.is_empty());
    assert_eq!(session.new_session_thinking_level, "auto");
}

/// 验证旧版应用配置会补齐 Git 与 Source Control 默认值。
///
/// 参数:
/// - 无
///
/// 返回:
/// - 无
#[test]
fn legacy_app_config_defaults_git_settings() {
    let mut value = serde_json::to_value(AppConfig::default()).unwrap();
    value.as_object_mut().unwrap().remove("git");
    value.as_object_mut().unwrap().remove("scm");

    let config: AppConfig = serde_json::from_value(value).unwrap();

    assert_eq!(config.scm.default_view_mode, "list");
    assert_eq!(config.git.untracked_changes, "separate");
    assert!(config.git.auto_repository_detection);
    assert!(config.git.detect_worktrees);
}

/// 验证反序列化会拒绝无效 Git 枚举和 worktree 限制。
///
/// 参数:
/// - 无
///
/// 返回:
/// - 无
#[test]
fn git_settings_reject_invalid_values() {
    let mut value = serde_json::to_value(AppConfig::default()).unwrap();
    value["git"]["untracked_changes"] = serde_json::json!("unknown");
    assert!(serde_json::from_value::<AppConfig>(value.clone()).is_err());

    value["git"]["untracked_changes"] = serde_json::json!("separate");
    value["git"]["detect_worktrees_limit"] = serde_json::json!(0);
    assert!(serde_json::from_value::<AppConfig>(value).is_err());
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
fn validate_rejects_invalid_compaction_ratio() {
    let mut config = AppConfig::default();
    config.context.compaction_ratio = 0.2;
    assert!(config.validate().is_err());
    config.context.compaction_ratio = 0.9;
    config.context.compaction_reserve_tokens = 8_000;
    assert!(config.validate().is_ok());
}

#[test]
fn parses_compaction_ratio_percent_or_fraction() {
    assert_eq!(
        crate::config::parse_compaction_ratio_text("90").unwrap(),
        0.9
    );
    assert_eq!(
        crate::config::parse_compaction_ratio_text("0.85").unwrap(),
        0.85
    );
    assert_eq!(
        crate::config::parse_compaction_ratio_text("95%").unwrap(),
        0.95
    );
    assert!(crate::config::parse_compaction_ratio_text("20").is_err());
}

#[test]
fn validate_rejects_invalid_temperature_and_timeout() {
    let mut config = AppConfig::default();
    config.providers[0].temperature = Some(3.0);
    assert!(config.validate().is_err());
    config.providers[0].temperature = Some(0.7);
    config.providers[0].timeout_seconds = 0;
    assert!(config.validate().is_err());
    config.providers[0].timeout_seconds = 60;
    config.providers[0].anthropic_max_tokens = 0;
    assert!(config.validate().is_err());
}

/// 【会话】【配置校验】验证内置内核的新会话默认值必须指向已配置模型。
///
/// 参数:
/// - 无
///
/// 返回:
/// - 无
#[test]
fn new_session_defaults_require_configured_native_model() {
    let mut config = AppConfig::default();
    let provider_id = config.providers[0].id.clone();
    let model = config.providers[0].default_model.clone();
    config.session.new_session_provider_id = provider_id.clone();
    config.session.new_session_model = model;

    assert!(config.validate().is_ok());

    config.session.new_session_model = "missing-model".to_string();
    let error = config.validate().unwrap_err();
    assert!(error
        .to_string()
        .contains("new-session model is not configured"));
}

/// 【会话】【配置校验】验证外部内核使用 ACP 虚拟供应商保存新会话模型。
///
/// 参数:
/// - 无
///
/// 返回:
/// - 无
#[test]
fn new_session_defaults_accept_acp_model_for_external_engine() {
    let mut config = AppConfig::default();
    config.agent.engine = AgentEngineKind::Codex;
    config.session.new_session_provider_id = "__acp__".to_string();
    config.session.new_session_model = "gpt-5.2-codex".to_string();

    assert!(config.validate().is_ok());

    config.session.new_session_provider_id = config.providers[0].id.clone();
    let error = config.validate().unwrap_err();
    assert!(error.to_string().contains("must be __acp__"));
}

/// 【会话】【配置校验】验证新会话思考等级拒绝未知值。
///
/// 参数:
/// - 无
///
/// 返回:
/// - 无
#[test]
fn new_session_defaults_reject_invalid_thinking_level() {
    let mut config = AppConfig::default();
    config.session.new_session_thinking_level = "extreme".to_string();

    let error = config.validate().unwrap_err();
    assert!(error
        .to_string()
        .contains("session.new_session_thinking_level is invalid"));
}

/// 【会话】【配置校验】验证新会话供应商与模型不能只配置其中一项。
///
/// 参数:
/// - 无
///
/// 返回:
/// - 无
#[test]
fn new_session_defaults_require_complete_model_pair() {
    let mut config = AppConfig::default();
    config.session.new_session_provider_id = config.providers[0].id.clone();

    let error = config.validate().unwrap_err();
    assert!(error.to_string().contains("must be provided together"));
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

/// 【配置测试】【业务拆分】主配置不再保存可拆卸插件的旧字段
/// @returns 无；独立插件设置只属于 plugins.jsonc
#[test]
fn removable_business_settings_are_absent_from_main_config() {
    let value = serde_json::to_value(AppConfig::default()).unwrap();
    for name in [
        "memes",
        "knowledge_base",
        "web_search",
        "web_images",
        "image_generation",
    ] {
        assert!(value["plugins"].get(name).is_none(), "{name}");
    }
    assert!(value.get("notification").is_none());
}

#[test]
fn paste_image_key_parses_every_configured_value() {
    assert_eq!(PasteImageKey::parse("ctrl_v"), Some(PasteImageKey::CtrlV));
    assert_eq!(PasteImageKey::parse("alt_v"), Some(PasteImageKey::AltV));
    assert_eq!(PasteImageKey::parse("both"), Some(PasteImageKey::Both));
    // 大小写与连字符写法都要认，配置是手写的
    assert_eq!(PasteImageKey::parse(" CTRL+V "), Some(PasteImageKey::CtrlV));
    assert_eq!(PasteImageKey::parse("alt+v"), Some(PasteImageKey::AltV));
    assert_eq!(PasteImageKey::parse("shift_v"), None);
}

#[test]
fn paste_image_key_round_trips_through_config_text() {
    for key in [
        PasteImageKey::CtrlV,
        PasteImageKey::AltV,
        PasteImageKey::Both,
    ] {
        assert_eq!(PasteImageKey::parse(key.as_str()), Some(key));
    }
}

/// Windows 默认必须避开被终端吞掉的 Ctrl+V。
#[test]
fn paste_image_key_default_avoids_ctrl_v_on_windows() {
    let expected = if cfg!(windows) {
        PasteImageKey::AltV
    } else {
        PasteImageKey::CtrlV
    };
    assert_eq!(PasteImageKey::default(), expected);
    assert_eq!(InputConfig::default().paste_image_key, expected);
}
