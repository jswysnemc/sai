use super::support::{runtime, FixtureHost};
use crate::{config::AppConfig, paths::SaiPaths};
use sai_plugin_runtime::ToolAccess;
use serde_json::Value;
use std::sync::Arc;

/// 【网页迁移测试】【模型契约】新包保留冻结原版的名称、说明、Schema 和只读分类
/// @returns 无；实际发布清单仅注册一个工具，不引入管理命令
#[test]
fn web_fetch_preserves_the_frozen_native_definition() {
    let plugin = runtime("web-fetch", Arc::new(FixtureHost::default()));
    let expected: Value =
        serde_json::from_str(include_str!("fixtures/web_fetch_definition.json")).unwrap();
    assert_eq!(plugin.tools().len(), 1);
    assert!(plugin.commands().is_empty());
    let tool = &plugin.tools()[0];
    assert_eq!(tool.name, expected["name"]);
    assert_eq!(tool.description, expected["description"]);
    assert_eq!(tool.parameters, expected["parameters"]);
    assert_eq!(tool.access, ToolAccess::ReadOnly);
}

/// 【网页迁移测试】【真实注册】普通和只读目录中的工具都必须归属 Lua 包
/// @returns 无；新包通过真实发现与注册，不能保留同名原生业务
#[test]
fn web_fetch_is_owned_by_lua_in_both_registries() {
    let root = tempfile::tempdir().unwrap();
    let paths = SaiPaths::for_tests(root.path());
    let config = AppConfig::default();
    super::example_support::install("web-fetch", &paths);
    crate::plugins::set_enabled(
        &config,
        &paths,
        "web-fetch",
        true,
        crate::plugins::GrantUpdate::Keep,
    )
    .unwrap();
    for registry in [
        crate::tools::builtin_registry_without_mcp(&config, &paths),
        crate::tools::readonly_registry(&config, &paths),
    ] {
        assert!(
            registry.plugin_diagnostics().is_empty(),
            "{:?}",
            registry.plugin_diagnostics()
        );
        assert!(!registry.contains("web_fetch"));
        assert_eq!(
            registry.plugin_owner("lua__web-fetch__web_fetch"),
            Some("web-fetch")
        );
    }
}
