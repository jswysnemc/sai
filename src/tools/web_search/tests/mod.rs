mod config;
mod contracts;
mod network;
mod support;

use super::*;
use crate::{config::AppConfig, paths::SaiPaths};

/// 【网页搜索测试】【入口注册】无参数；验证普通、只读和 CLI 入口独立于技能且遵守总开关。
#[test]
fn web_search_is_native_with_skills_disabled() {
    let root = tempfile::tempdir().unwrap();
    let paths = SaiPaths::for_tests(root.path());
    let mut config = AppConfig::default();
    config.skills.enabled = false;
    config.skills.allow_command_execution = false;
    for enabled in [true, false] {
        config.plugins.web.enabled = enabled;
        for mode in [crate::agent::AgentMode::Yolo, crate::agent::AgentMode::Plan] {
            let registry =
                crate::cli::build_tool_registry_with_cached_mcp(&config, &paths, mode).unwrap();
            assert_eq!(registry.contains("web_search"), enabled);
            assert_eq!(registry.plugin_owner("web_search"), None);
            if enabled {
                let tool = registry
                    .tool_infos()
                    .into_iter()
                    .find(|tool| tool.name == "web_search")
                    .unwrap();
                assert_eq!(tool.permission, crate::tools::ToolPermission::ReadOnly);
                registry
                    .validate_arguments("web_search", r#"{"query":"Rust","provider":"script"}"#)
                    .unwrap();
                assert!(registry
                    .validate_arguments("web_search", r#"{"query":"Rust","provider":"unknown"}"#)
                    .is_err());
            }
        }
        let entries = crate::tools::tool_catalog(&config, &paths);
        let entry = entries
            .iter()
            .find(|entry| entry.name == "web_search")
            .unwrap();
        assert_eq!(entry.group, "web");
        assert!(!entry.resident);
    }
}

/// 【网页搜索测试】【旧技能隔离】无参数；验证旧脚本不能覆盖原生工具或绕过总开关。
#[test]
fn old_web_search_skill_cannot_override_native_search() {
    let root = tempfile::tempdir().unwrap();
    let paths = SaiPaths::for_tests(root.path());
    let directory = paths.skills_dir.join("web-search");
    std::fs::create_dir_all(directory.join("scripts")).unwrap();
    std::fs::write(
        directory.join("SKILL.md"),
        "---\nname: web-search\n---\nLegacy skill",
    )
    .unwrap();
    std::fs::write(
        directory.join("scripts/web-search.py"),
        "raise RuntimeError('legacy script')",
    )
    .unwrap();
    let mut config = AppConfig::default();
    let registry = crate::cli::build_tool_registry_with_cached_mcp(
        &config,
        &paths,
        crate::agent::AgentMode::Yolo,
    )
    .unwrap();
    let parameters = registry
        .definition("web_search")
        .unwrap()
        .function
        .parameters;
    assert!(parameters["properties"].get("location").is_some());
    config.plugins.web.enabled = false;
    let registry = crate::cli::build_tool_registry_with_cached_mcp(
        &config,
        &paths,
        crate::agent::AgentMode::Yolo,
    )
    .unwrap();
    assert!(!registry.contains("web_search"));
}
