use super::{operations::*, *};

/// 【模型接入】【持久化回归】编辑连接后多密钥、稳定标识和目录保持不变，配置可重新读取。
/// 参数: 无；返回: 无
#[test]
fn editing_preserves_keys_and_roundtrips() {
    let mut config = AppConfig::default();
    let mut item = new_endpoint(&config, ModelEndpointKind::Jev);
    item.api_keys = vec![crate::config::ProviderApiKey {
        id: "stable".into(),
        api_key: "fixture-key".into(),
        label: "primary".into(),
    }];
    item.api_key_selected = Some("stable".into());
    item.models = vec!["jev-latest".into()];
    save(&mut config, item.clone()).unwrap();
    activate(&mut config, &item.id);
    item.endpoint = "http://127.0.0.1:9087/custom".into();
    save(&mut config, item.clone()).unwrap();
    let directory = tempfile::tempdir().unwrap();
    let paths = crate::paths::SaiPaths::for_tests(directory.path());
    paths.create_dirs().unwrap();
    config.save(&paths).unwrap();
    let loaded = AppConfig::load_or_default(&paths).unwrap();
    assert_eq!(loaded.jev_endpoint().unwrap(), Some(&item));
    assert_eq!(loaded.jev_connection().unwrap().api_key, "fixture-key");
}

/// 【模型接入】【默认选择回归】切换和删除操作遵循运行时默认选择规则。
/// 参数: 无；返回: 无
#[test]
fn default_selection_and_deletion_are_consistent() {
    let mut config = AppConfig::default();
    for kind in [ModelEndpointKind::ImageGeneration, ModelEndpointKind::Jev] {
        let first = new_endpoint(&config, kind);
        save(&mut config, first.clone()).unwrap();
        let second = new_endpoint(&config, kind);
        save(&mut config, second.clone()).unwrap();
        activate(&mut config, &second.id);
        assert_eq!(active_id(&config, kind), Some(second.id.as_str()));
        remove(&mut config, &second.id);
        assert_eq!(active_id(&config, kind), Some(first.id.as_str()));
        config.validate().unwrap();
    }
}

/// 【模型接入】【事务回归】非法地址不能覆盖已保存接入。
/// 参数: 无；返回: 无
#[test]
fn invalid_draft_does_not_mutate_config() {
    let mut config = AppConfig::default();
    let item = new_endpoint(&config, ModelEndpointKind::Jev);
    save(&mut config, item.clone()).unwrap();
    let mut invalid = item.clone();
    invalid.endpoint = "file:///tmp/model".into();
    assert!(save(&mut config, invalid).is_err());
    assert_eq!(config.model_endpoints, vec![item]);
}
