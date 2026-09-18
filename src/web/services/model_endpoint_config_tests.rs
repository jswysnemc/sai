use super::config_service::{load_redacted, save, SECRET_SENTINEL};
use crate::{config::AppConfig, paths::SaiPaths};
use serde_json::json;

/// 【模型接入】【配置往返】独立端点和密钥可保存、重排、替换和清除，普通供应商不变。
/// 参数: 无；返回无
#[test]
fn specialized_endpoints_round_trip_without_changing_chat_providers() {
    let root = tempfile::tempdir().unwrap();
    let paths = SaiPaths::for_tests(root.path());
    AppConfig::init_files(&paths).unwrap();
    let mut input = load_redacted(&paths).unwrap();
    let original_providers = input["providers"].clone();
    let original_active = input["active_provider"].clone();
    input["model_endpoints"] = json!([
        {"id":"image", "kind":"image_generation", "name":"Image", "endpoint":"https://images.example.com:8443/v2/generate", "api_key":"image-key", "model":"custom-image"},
        {"id":"jev", "kind":"jev", "name":"JEV", "endpoint":"http://localhost:9087/decide", "api_key":"jev-key", "model":"custom-jev"}
    ]);
    let mut saved = save(&paths, input).unwrap();
    assert_eq!(saved["providers"], original_providers);
    assert_eq!(saved["active_provider"], original_active);
    assert_eq!(saved["model_endpoints"][0]["api_key"], SECRET_SENTINEL);
    assert_eq!(saved["model_endpoints"][1]["api_key"], SECRET_SENTINEL);
    saved["model_endpoints"].as_array_mut().unwrap().reverse();
    let mut saved = save(&paths, saved).unwrap();
    let config = AppConfig::load(&paths).unwrap();
    assert_eq!(config.model_endpoints[0].api_key, "jev-key");
    assert_eq!(config.model_endpoints[1].api_key, "image-key");
    assert_eq!(
        config.model_endpoints[1].endpoint,
        "https://images.example.com:8443/v2/generate"
    );
    saved["model_endpoints"][0]["api_key"] = json!("");
    saved["model_endpoints"][1]["api_key"] = json!("replacement-key");
    save(&paths, saved).unwrap();
    let config = AppConfig::load(&paths).unwrap();
    assert!(config.model_endpoints[0].api_key.is_empty());
    assert_eq!(config.model_endpoints[1].api_key, "replacement-key");
    assert!(!config
        .provider_model_choices()
        .iter()
        .any(|choice| choice.model == "custom-jev" || choice.model == "custom-image"));
}

/// 【模型接入】【保存校验】坏地址和重复标识不能覆盖已保存配置；无参数，无返回值
#[test]
fn invalid_endpoint_save_does_not_modify_disk() {
    let root = tempfile::tempdir().unwrap();
    let paths = SaiPaths::for_tests(root.path());
    AppConfig::init_files(&paths).unwrap();
    let original = std::fs::read(&paths.config_file).unwrap();
    let mut config = load_redacted(&paths).unwrap();
    config["model_endpoints"] = json!([{"id":"bad", "kind":"jev", "name":"Bad", "endpoint":"bad-url", "api_key":"secret", "model":""}]);
    assert!(save(&paths, config).is_err());
    assert_eq!(std::fs::read(&paths.config_file).unwrap(), original);
}
