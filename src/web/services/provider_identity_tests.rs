use super::config_service::{load_redacted, save, SECRET_SENTINEL};
use crate::config::AppConfig;
use crate::paths::SaiPaths;
use serde_json::json;

/// 【供应商配置】【改名回归】连续改名并重排后，仍按原对象保存密钥且不持久化原始 ID。
#[test]
fn renamed_provider_keeps_its_own_credentials() {
    let root = tempfile::tempdir().unwrap();
    let paths = SaiPaths::for_tests(root.path());
    let mut config = AppConfig::default();
    config.providers[0].api_key = Some("fixture-first".into());
    config.providers[1].api_key = Some("fixture-second".into());
    config.save(&paths).unwrap();
    let original_id = config.providers[1].id.clone();
    let mut submitted = load_redacted(&paths).unwrap();
    submitted["providers"][1]["original_id"] = json!(original_id);
    submitted["providers"][1]["id"] = json!("renamed-provider");
    submitted["providers"].as_array_mut().unwrap().swap(0, 1);
    let saved = save(&paths, submitted).unwrap();
    assert_eq!(saved["providers"][0]["api_key"], SECRET_SENTINEL);
    let reloaded = AppConfig::load(&paths).unwrap();
    assert_eq!(
        reloaded.providers[0].api_key.as_deref(),
        Some("fixture-second")
    );
    assert_eq!(
        reloaded.providers[1].api_key.as_deref(),
        Some("fixture-first")
    );
    assert!(!std::fs::read_to_string(&paths.config_file)
        .unwrap()
        .contains("original_id"));
}

/// 【供应商配置】【密钥隔离】未标注来源的新 ID 不得从数组原位置继承密钥。
#[test]
fn new_provider_cannot_inherit_secret_by_position() {
    let root = tempfile::tempdir().unwrap();
    let paths = SaiPaths::for_tests(root.path());
    let mut config = AppConfig::default();
    config.providers[0].api_key = Some("fixture-first".into());
    config.save(&paths).unwrap();
    let mut submitted = load_redacted(&paths).unwrap();
    submitted["providers"][0]
        .as_object_mut()
        .unwrap()
        .remove("original_id");
    submitted["providers"][0]["id"] = json!("brand-new");
    assert!(save(&paths, submitted).is_err());
}

/// 【供应商配置】【改名探测】新 ID 草稿仍能恢复单密钥迁移后的 key-1 和敏感请求头。
#[test]
fn renamed_draft_restores_legacy_key_and_headers_before_probe() {
    let root = tempfile::tempdir().unwrap();
    let paths = SaiPaths::for_tests(root.path());
    let mut config = AppConfig::default();
    config.providers[0].api_key = Some("fixture-key".into());
    config.providers[0]
        .extra_headers
        .insert("X-API-Key".into(), "fixture-header".into());
    config.save(&paths).unwrap();
    let mut value = load_redacted(&paths).unwrap()["providers"][0].clone();
    value["id"] = json!("renamed");
    value["api_key"] = json!("");
    value["api_keys"] = json!([{ "id": "key-1", "api_key": SECRET_SENTINEL }]);
    let draft: super::provider_identity::ProviderDraft = serde_json::from_value(value).unwrap();
    let restored = draft.restore(&paths).unwrap();
    assert_eq!(restored.id, "renamed");
    assert_eq!(restored.api_keys[0].api_key, "fixture-key");
    assert_eq!(restored.extra_headers["X-API-Key"], "fixture-header");
}

/// 【供应商配置】【过期来源】来源已经删除或被重复使用时拒绝保存，不覆盖现有配置。
#[test]
fn missing_and_duplicate_sources_are_rejected() {
    let root = tempfile::tempdir().unwrap();
    let paths = SaiPaths::for_tests(root.path());
    AppConfig::default().save(&paths).unwrap();
    let mut submitted = load_redacted(&paths).unwrap();
    submitted["providers"][1]["original_id"] = submitted["providers"][0]["original_id"].clone();
    assert!(save(&paths, submitted).is_err());
    let mut submitted = load_redacted(&paths).unwrap();
    submitted["providers"][0]["original_id"] = json!("deleted-provider");
    assert!(save(&paths, submitted).is_err());
}
