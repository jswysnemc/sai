//! 首次供应商引导的持久化、校验、凭据与旧配置兼容测试。

use super::onboarding::{complete_provider_setup, ProviderSetupInput};
use super::AppConfig;
use crate::paths::SaiPaths;
use std::path::Path;

/// 【首次配置测试】【隔离路径】在临时目录构造应用路径，避免接触用户配置。
/// @param root 为临时目录
/// @returns 测试路径集合
fn paths(root: &Path) -> SaiPaths {
    SaiPaths {
        config_dir: root.join("config"),
        config_file: root.join("config/config.jsonc"),
        secrets_file: root.join("config/secrets.jsonc"),
        skills_dir: root.join("config/skills"),
        data_dir: root.join("data"),
        cache_dir: root.join("cache"),
        state_dir: root.join("state"),
        pictures_dir: root.join("pictures"),
        fish_hook_file: root.join("fish/sai.fish"),
        bash_hook_file: root.join("shell/bash-hook.sh"),
        zsh_hook_file: root.join("shell/zsh-hook.zsh"),
        powershell_hook_file: root.join("shell/sai.ps1"),
    }
}

/// 【首次配置测试】【表单样本】创建不依赖外部网络的自定义供应商表单。
/// @returns 连接配置样本
fn input() -> ProviderSetupInput {
    ProviderSetupInput {
        provider_id: None,
        display_name: "Local test".into(),
        base_url: " http://127.0.0.1:11434/v1/chat/completions/ ".into(),
        protocol: "openai-chat".into(),
        api_key: Some("local-fixture".into()),
        model: "test-model".into(),
    }
}

/// 【首次配置测试】【初始化】新配置持续等待引导，重复初始化不能误标为已完成。
/// @returns 无
#[test]
fn fresh_configuration_requires_setup_until_completed() {
    let temp = tempfile::tempdir().unwrap();
    let paths = paths(temp.path());
    for _ in 0..2 {
        AppConfig::init_files(&paths).unwrap();
        assert!(!AppConfig::load(&paths).unwrap().provider_setup_complete);
    }
}

/// 【首次配置测试】【旧配置】没有状态字段的已有配置不在升级后重新引导。
/// @returns 无
#[test]
fn legacy_configuration_remains_completed() {
    let mut legacy = serde_json::to_value(AppConfig::default()).unwrap();
    legacy
        .as_object_mut()
        .unwrap()
        .remove("provider_setup_complete");
    let config: AppConfig = serde_json::from_value(legacy).unwrap();
    assert!(config.provider_setup_complete);
}

/// 【首次配置测试】【共享完成】保存供应商、默认模型和共享状态，重新加载后仍生效。
/// @returns 无
#[test]
fn setup_persists_provider_and_shared_completion() {
    let temp = tempfile::tempdir().unwrap();
    let paths = paths(temp.path());
    AppConfig::init_files(&paths).unwrap();
    let mut config = AppConfig::load(&paths).unwrap();
    config.display.repl_transcript_row_cap = 333;
    config.save(&paths).unwrap();
    complete_provider_setup(&paths, input()).unwrap();
    let saved = AppConfig::load(&paths).unwrap();
    assert!(saved.provider_setup_complete);
    assert_eq!(saved.active_provider, "custom");
    assert_eq!(saved.session.new_session_provider_id, "custom");
    assert_eq!(saved.session.new_session_model, "test-model");
    assert_eq!(saved.display.repl_transcript_row_cap, 333);
    let provider = saved.provider(None).unwrap();
    assert_eq!(provider.base_url, "http://127.0.0.1:11434/v1");
    assert_eq!(provider.resolved_api_key(&paths).unwrap(), "local-fixture");
    assert!(provider.models.contains(&"test-model".to_string()));
    // 1. 请求重试不重复创建供应商，也不覆盖已经完成的配置
    let again = complete_provider_setup(&paths, input()).unwrap();
    assert_eq!(again.providers.len(), saved.providers.len());
}

/// 【首次配置测试】【失败不写入】无效地址、缺少模型或缺少密钥时保留未完成状态。
/// @returns 无
#[test]
fn invalid_setup_does_not_write_partial_configuration() {
    let temp = tempfile::tempdir().unwrap();
    let paths = paths(temp.path());
    AppConfig::init_files(&paths).unwrap();
    let original = std::fs::read(&paths.config_file).unwrap();
    for invalid in 0..5 {
        let mut draft = input();
        match invalid {
            0 => draft.base_url = "file:///tmp/api".into(),
            1 => draft.model.clear(),
            2 => draft.api_key = None,
            3 => draft.protocol = "unknown".into(),
            _ => draft.display_name.clear(),
        }
        assert!(complete_provider_setup(&paths, draft).is_err());
        assert_eq!(std::fs::read(&paths.config_file).unwrap(), original);
    }
}

/// 【首次配置测试】【免费供应商】用户明确确认内置供应商后，无密钥也可以完成引导。
/// @returns 无
#[test]
fn built_in_provider_requires_confirmation_but_no_key() {
    let temp = tempfile::tempdir().unwrap();
    let paths = paths(temp.path());
    AppConfig::init_files(&paths).unwrap();
    let config = AppConfig::load(&paths).unwrap();
    let provider = config.provider(None).unwrap();
    let draft = ProviderSetupInput {
        provider_id: Some(provider.id.clone()),
        display_name: provider.display_name.clone(),
        base_url: provider.base_url.clone(),
        protocol: provider.protocol.clone(),
        api_key: None,
        model: provider.default_model.clone(),
    };
    assert!(
        complete_provider_setup(&paths, draft)
            .unwrap()
            .provider_setup_complete
    );
}

/// 【首次配置测试】【凭据来源】空白输入保留独立密钥文件，错误环境变量不得跳过校验。
/// @returns 无
#[test]
fn setup_resolves_existing_secrets_and_rejects_missing_environment() {
    let temp = tempfile::tempdir().unwrap();
    let paths = paths(temp.path());
    AppConfig::init_files(&paths).unwrap();
    let mut draft = input();
    draft.provider_id = Some("openai".into());
    draft.api_key = Some(format!(
        "$env:SAI_SETUP_MISSING_{}",
        uuid::Uuid::new_v4().simple()
    ));
    assert!(complete_provider_setup(&paths, draft.clone()).is_err());
    std::fs::write(
        &paths.secrets_file,
        r#"{"api_keys":{"openai":"stored-fixture"}}"#,
    )
    .unwrap();
    draft.api_key = None;
    let config = complete_provider_setup(&paths, draft).unwrap();
    assert_eq!(
        config
            .provider(None)
            .unwrap()
            .resolved_api_key(&paths)
            .unwrap(),
        "stored-fixture"
    );
}
