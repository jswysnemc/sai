mod network;
mod presets;
mod support;

use super::*;
use crate::{config::AppConfig, paths::SaiPaths, tools::ToolPermission};

/// 【网页读取测试】【原生入口】无参数；验证禁用技能和网页搜索后，普通及 Plan 目录仍提供原生只读工具。
#[test]
fn web_fetch_is_native_and_keeps_public_schema() {
    let root = tempfile::tempdir().unwrap();
    let paths = SaiPaths::for_tests(root.path());
    let mut config = AppConfig::default();
    config.skills.enabled = false;
    config.plugins.web.enabled = false;
    let expected: Value = serde_json::from_str(include_str!(
        "../../../plugins/tests/fixtures/web_fetch_definition.json"
    ))
    .unwrap();
    for mode in [crate::agent::AgentMode::Yolo, crate::agent::AgentMode::Plan] {
        let registry =
            crate::cli::build_tool_registry_with_cached_mcp(&config, &paths, mode).unwrap();
        assert!(registry.contains("web_fetch"));
        assert_eq!(registry.plugin_owner("web_fetch"), None);
        let definition = registry.definition("web_fetch").unwrap().function;
        assert_eq!(definition.name, expected["name"]);
        assert!(definition.description.contains("HTTP(S) URL"));
        assert!(definition.description.contains("JavaScript"));
        assert_eq!(definition.parameters, expected["parameters"]);
        assert_eq!(
            registry.permission("web_fetch").unwrap(),
            ToolPermission::ReadOnly
        );
        assert!(registry
            .validate_arguments(
                "web_fetch",
                r#"{"url":"https://example.com","format":"pdf"}"#
            )
            .is_err());
    }
    let entries = crate::tools::tool_catalog(&config, &paths);
    let entry = entries
        .iter()
        .find(|entry| entry.name == "web_fetch")
        .unwrap();
    assert_eq!(entry.group, "web");
}

/// 【网页读取测试】【输入边界】无参数；非法协议拒绝，字符上限与超时保持原有整数规则。
#[test]
fn web_fetch_validates_url_and_normalizes_limits() {
    for url in [
        "",
        "file:///etc/passwd",
        "ftp://example.com",
        "data:text/plain,x",
        "https://[bad?token=secret",
    ] {
        let error = request::FetchInput::parse(&json!({"url":url}))
            .err()
            .unwrap();
        assert!(!error.to_string().contains("secret"));
    }
    for (value, expected) in [
        (json!(0), 1),
        (json!(-1), 24000),
        (json!(3.0), 24000),
        (json!(u64::MAX), 80000),
    ] {
        let input = request::FetchInput::parse(
            &json!({"url":" https://example.com ","max_chars":value,"timeout":u64::MAX}),
        )
        .unwrap();
        assert_eq!(input.max_chars, expected);
        assert_eq!(input.timeout, 120);
    }
}
