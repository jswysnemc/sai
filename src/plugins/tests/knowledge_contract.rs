use super::support::FixtureHost;
use crate::{config::AppConfig, paths::SaiPaths};
use sai_plugin_runtime::{PluginRuntime, ToolAccess};
use serde_json::json;
use std::sync::Arc;

pub(super) const READ_TOOLS: [&str; 3] = [
    "search_knowledge_base",
    "search_knowledge_base_by_name",
    "read_knowledge_base_file",
];
pub(super) const WRITE_TOOLS: [&str; 3] = [
    "upload_text_to_knowledge_base",
    "edit_knowledge_base_file",
    "remove_knowledge_base_file",
];

/// 【知识库迁移测试】【完整发布】六工具与全部管理入口必须来自同一 Lua 包
/// @returns 无；工具权限、命令目录和后台入口全部匹配
#[test]
fn knowledge_publishes_tools_and_management_commands() {
    let package = super::example_support::package("knowledge-base");
    let grants = package.manifest.capabilities.clone();
    let runtime =
        PluginRuntime::load(package, json!({}), grants, Arc::new(FixtureHost::default())).unwrap();
    assert_eq!(runtime.tools().len(), 6);
    for (names, access) in [
        (READ_TOOLS, ToolAccess::ReadOnly),
        (WRITE_TOOLS, ToolAccess::Writes),
    ] {
        for name in names {
            let tool = runtime
                .tools()
                .iter()
                .find(|tool| tool.name == name)
                .unwrap();
            assert_eq!(tool.access, access, "{name}");
        }
    }
    let mut commands = runtime
        .commands()
        .iter()
        .map(|command| command.name.as_str())
        .collect::<Vec<_>>();
    commands.sort();
    assert_eq!(
        commands,
        [
            "add",
            "add-text",
            "embed-reindex",
            "find",
            "list",
            "read",
            "reindex",
            "remove",
            "search",
            "stats"
        ]
    );
}

/// 【知识库迁移测试】【模型契约】六项描述和参数来自冻结原版，保留用途、删除确认及分页说明
/// @returns 无；工具说明、JSON schema 和写入分类均匹配原版
#[test]
fn knowledge_preserves_all_native_tool_definitions() {
    let package = super::example_support::package("knowledge-base");
    let runtime = PluginRuntime::load(
        package.clone(),
        json!({}),
        package.manifest.capabilities,
        Arc::new(FixtureHost::default()),
    )
    .unwrap();
    let expected: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/knowledge_definitions.json")).unwrap();
    for definition in expected.as_array().unwrap() {
        let actual = runtime
            .tools()
            .iter()
            .find(|tool| tool.name == definition["name"].as_str().unwrap())
            .unwrap();
        assert_eq!(
            actual.description,
            definition["description"].as_str().unwrap()
        );
        assert_eq!(actual.parameters, definition["parameters"]);
        assert_eq!(
            actual.access == ToolAccess::Writes,
            definition["writes"].as_bool().unwrap()
        );
    }
}

/// 【知识库迁移测试】【真实归属】普通安装与独立开关正确限制工具目录
/// @returns 无；禁用和关闭上传时不会暴露对应写入工具
#[test]
fn knowledge_registry_uses_installed_lua_and_independent_settings() {
    let root = tempfile::tempdir().unwrap();
    let paths = SaiPaths::for_tests(root.path());
    let config = AppConfig::default();
    super::example_support::install("knowledge-base", &paths);
    for enabled in [true, false] {
        for upload in [true, false] {
            crate::plugins::configure(
                &config,
                &paths,
                "knowledge-base",
                json!({"upload_tool_enabled":upload}),
            )
            .unwrap();
            crate::plugins::set_enabled(
                &config,
                &paths,
                "knowledge-base",
                enabled,
                crate::plugins::GrantUpdate::Declared,
            )
            .unwrap();
            let normal = crate::tools::builtin_registry_without_mcp(&config, &paths);
            let readonly = crate::tools::readonly_registry(&config, &paths);
            assert!(
                normal.plugin_diagnostics().is_empty(),
                "{:?}",
                normal.plugin_diagnostics()
            );
            assert!(readonly.plugin_diagnostics().is_empty());
            for name in READ_TOOLS.into_iter().chain(WRITE_TOOLS) {
                let expected = enabled && (READ_TOOLS.contains(&name) || upload);
                let public = format!("lua__knowledge-base__{name}");
                assert_eq!(normal.contains(&public), expected, "{name}");
                assert_eq!(
                    readonly.contains(&public),
                    enabled && READ_TOOLS.contains(&name),
                    "{name}"
                );
                if expected {
                    assert_eq!(
                        normal.plugin_owner(&public),
                        Some("knowledge-base"),
                        "{name}"
                    );
                }
            }
        }
    }
}
