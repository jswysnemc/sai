use super::*;

/// 构造测试用 JEV 接入。
fn endpoint(id: &str, url: &str, key: &str) -> ModelEndpointConfig {
    ModelEndpointConfig {
        id: id.into(),
        kind: ModelEndpointKind::Jev,
        name: format!("{id} name"),
        endpoint: url.into(),
        protocol: "auto".into(),
        api_key: key.into(),
        api_keys: Vec::new(),
        api_key_selected: None,
        api_key_balance: false,
        models: Vec::new(),
        model: String::new(),
    }
}

#[test]
fn partial_json_keeps_defaults() {
    let config: JevConfig = serde_json::from_str(r#"{"routing":{"enabled":true}}"#).unwrap();
    assert!(config.routing.enabled);
    assert_eq!(config.routing.max_tools, 6);
    assert!(!config.audit.enabled);
    assert_eq!(config.audit.minimum_probability, 0.9);
}

#[test]
fn root_url_is_completed_to_systemone() {
    let info = jev_connection_info_for(Some(&endpoint("a", "https://api.typesafe.ai/v1/", "k")));
    assert_eq!(info.endpoint, JEV_OFFICIAL_ENDPOINT);
    assert_eq!(info.model, JEV_DEFAULT_MODEL);
    let custom = jev_connection_info_for(Some(&endpoint("b", "http://localhost:9087/decide", "k")));
    assert_eq!(custom.endpoint, "http://localhost:9087/decide");
}

#[test]
fn endpoint_key_is_used_before_environment() {
    let connection = jev_connection_for(Some(&endpoint("a", JEV_OFFICIAL_ENDPOINT, " own "))).unwrap();
    assert_eq!(connection.api_key, "own");
    assert_eq!(connection.info.source, JevConnectionSource::Endpoint);
}

#[test]
fn missing_env_reference_is_reported() {
    let item = endpoint("a", JEV_OFFICIAL_ENDPOINT, "$env:SAI_JEV_TEST_MISSING_KEY");
    let error = jev_connection_for(Some(&item)).unwrap_err().to_string();
    assert!(error.contains("SAI_JEV_TEST_MISSING_KEY"));
}

#[test]
fn first_jev_endpoint_is_used_when_id_is_empty() {
    let mut config = AppConfig::default();
    assert!(config.jev_endpoint().unwrap().is_none());
    config.model_endpoints = vec![endpoint("one", JEV_OFFICIAL_ENDPOINT, "k"), endpoint("two", JEV_OFFICIAL_ENDPOINT, "k")];
    assert_eq!(config.jev_endpoint().unwrap().unwrap().id, "one");
    config.jev.endpoint_id = "two".into();
    assert_eq!(config.jev_endpoint().unwrap().unwrap().id, "two");
}

#[test]
fn validation_rejects_unknown_endpoint_and_bad_ranges() {
    let endpoints = vec![endpoint("one", JEV_OFFICIAL_ENDPOINT, "k")];
    let mut config = JevConfig::default();
    config.validate(&endpoints).unwrap();
    config.endpoint_id = "missing".into();
    assert!(config.validate(&endpoints).is_err());
    config.endpoint_id = "one".into();
    config.audit.minimum_probability = 0.3;
    assert!(config.validate(&endpoints).is_err());
    config.audit.minimum_probability = 0.9;
    config.routing.threshold = 1.5;
    assert!(config.validate(&endpoints).is_err());
}
