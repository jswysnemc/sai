use super::todo_support::{bundled, call, context, snapshot};
use crate::{
    config::AppConfig,
    paths::SaiPaths,
    plugins::{self, private::PrivatePluginHost, todo_view::TodoView, GrantUpdate, PluginSource},
    state::StateStore,
};
use sai_plugin_runtime::{PluginRuntime, ToolPolicyInput};
use serde_json::{json, Value};
use std::sync::Arc;

/// 【待办设置测试】【配置损坏】无效插件配置必须作为错误显示，不能伪装成没有待办
/// @returns 无；显式禁用仍是合法空快照
#[tokio::test]
async fn todo_view_reports_invalid_plugin_configuration() {
    let root = tempfile::tempdir().unwrap();
    let paths = SaiPaths::for_tests(root.path());
    std::fs::create_dir_all(&paths.config_dir).unwrap();
    std::fs::write(paths.config_dir.join("plugins.jsonc"), "{broken").unwrap();
    assert!(TodoView::load(&AppConfig::default(), &paths).await.is_err());
}

/// 【待办设置测试】【启停和计划模式】待办属于 Lua 写入工具，禁用同时移除工具和界面投影
/// @returns 无；重新启用读取原记录，白名单保留真实插件归属
#[tokio::test]
async fn todo_settings_control_tool_registration_and_web_visibility() {
    let root = tempfile::tempdir().unwrap();
    let (paths, plugin, _) = bundled(root.path());
    let config = AppConfig::default();
    let store = StateStore::new(&paths).unwrap();
    let scope = store.state_dir().display().to_string();
    call(
        &plugin,
        root.path(),
        &scope,
        json!({"action":"add","text":"preserved"}),
    )
    .await;
    let record = std::fs::read(store.state_dir().join("todos.plugin.json")).unwrap();
    for enabled in [false, true] {
        plugins::set_enabled(&config, &paths, "todo", enabled, GrantUpdate::Keep).unwrap();
        let registry = crate::tools::builtin_registry_without_mcp(&config, &paths);
        assert!(registry.plugin_diagnostics().is_empty());
        assert_eq!(registry.contains("todo"), enabled);
        let readonly = crate::tools::readonly_registry(&config, &paths);
        assert!(!readonly.contains("todo"));
        if enabled {
            assert_eq!(
                registry.clone_filtered(&["todo"]).plugin_owner("todo"),
                Some("todo")
            );
        }
        let view = TodoView::load(&config, &paths).await.unwrap();
        assert_eq!(
            view.snapshot(store.session_id(), store.state_dir(), root.path())
                .await
                .unwrap()
                .items
                .len(),
            usize::from(enabled)
        );
        assert_eq!(
            std::fs::read(store.state_dir().join("todos.plugin.json")).unwrap(),
            record
        );
    }
}

/// 【待办设置测试】【独立撤权】存储撤权拒绝读写，回复策略撤权不会取消仍获授权的工具
/// @returns 无；宿主写入上下文不能弥补缺失授权
#[tokio::test]
async fn todo_settings_enforce_independent_storage_and_policy_grants() {
    let root = tempfile::tempdir().unwrap();
    let paths = SaiPaths::for_tests(root.path());
    let store = StateStore::new(&paths).unwrap();
    let scope = store.state_dir().display().to_string();
    for (storage, policy) in [(false, true), (true, false)] {
        let mut descriptor = plugins::discover(&AppConfig::default(), &paths)
            .plugins
            .into_iter()
            .find(|item| item.package.manifest.id == "todo")
            .unwrap();
        let mut grants = descriptor.grants();
        grants.system.session_storage = storage;
        grants.reply_policy = policy;
        descriptor.setting.grants = Some(grants);
        let host = Arc::new(PrivatePluginHost::for_descriptor(&paths, &descriptor).unwrap());
        let plugin = PluginRuntime::load(
            descriptor.runtime_package(),
            descriptor.settings().clone(),
            descriptor.grants(),
            host,
        )
        .unwrap();
        let result = plugin
            .call_tool(
                "todo",
                json!({"action":"add","text":"allowed"}),
                context(root.path(), &scope, true),
            )
            .await;
        assert_eq!(result.is_ok(), storage);
        assert_eq!(
            plugin
                .call_command("snapshot", "", context(root.path(), &scope, false))
                .await
                .is_ok(),
            storage
        );
        let input = ToolPolicyInput {
            name: "step".into(),
            local_name: None,
            arguments: json!({}),
            ok: true,
            tools: vec!["todo".into()],
        };
        assert!(plugin
            .after_tool(input, Value::Null, context(root.path(), &scope, true))
            .await
            .is_err());
    }
}

/// 【待办设置测试】【来源隔离】同 ID 的外部描述符不能继承内置会话文件兼容权限
/// @returns 无；外部包只创建自身普通私有记录，旧待办原文不变
#[tokio::test]
async fn todo_external_descriptor_cannot_adopt_bundled_legacy_records() {
    let root = tempfile::tempdir().unwrap();
    let paths = SaiPaths::for_tests(root.path());
    let store = StateStore::new(&paths).unwrap();
    let scope = store.state_dir().display().to_string();
    let old = json!([super::todo_support::item("legacy", "pending")]).to_string();
    std::fs::write(store.state_dir().join("todos.json"), &old).unwrap();
    let mut descriptor = plugins::discover(&AppConfig::default(), &paths)
        .plugins
        .into_iter()
        .find(|item| item.package.manifest.id == "todo")
        .unwrap();
    descriptor.source = PluginSource::Installed(root.path().join("external"));
    descriptor.setting.grants = Some(descriptor.package.manifest.capabilities.clone());
    let host = Arc::new(PrivatePluginHost::for_descriptor(&paths, &descriptor).unwrap());
    let plugin = PluginRuntime::load(
        descriptor.runtime_package(),
        json!({}),
        descriptor.grants(),
        host,
    )
    .unwrap();
    assert_eq!(
        snapshot(&plugin, root.path(), &scope).await["items"],
        json!([])
    );
    call(
        &plugin,
        root.path(),
        &scope,
        json!({"action":"add","text":"external"}),
    )
    .await;
    assert!(!store.state_dir().join("todos.plugin.json").exists());
    assert_eq!(
        std::fs::read_to_string(store.state_dir().join("todos.json")).unwrap(),
        old
    );
}

/// 【待办设置测试】【工作区和入口隔离】同名会话按完整目录隔离，直接工具记录使用通用清理入口
/// @returns 无；清理直接入口不删除真实会话待办
#[tokio::test]
async fn todo_storage_isolates_workspaces_and_direct_cli_cleanup() {
    let root = tempfile::tempdir().unwrap();
    let (paths, plugin, _) = bundled(root.path());
    let mut scopes = Vec::new();
    for workspace in ["first", "second"] {
        let path = root.path().join(workspace);
        std::fs::create_dir(&path).unwrap();
        let store = StateStore::for_workspace_session(&paths, &path, "default").unwrap();
        let scope = store.state_dir().display().to_string();
        let mut ctx = context(&path, &scope, true);
        ctx.session_id = "default".into();
        plugin
            .call_tool("todo", json!({"action":"add","text":workspace}), ctx)
            .await
            .unwrap();
        scopes.push(scope);
    }
    for (number, expected) in ["first", "second"].iter().enumerate() {
        let items = snapshot(&plugin, root.path(), &scopes[number]).await;
        assert_eq!(items["items"].as_array().unwrap().len(), 1);
        assert_eq!(items["items"][0]["text"], *expected);
    }
    call(
        &plugin,
        root.path(),
        "cli-tool",
        json!({"action":"add","text":"direct"}),
    )
    .await;
    plugins::clear_session_storage(&paths.state_dir, "cli-tool").unwrap();
    assert_eq!(
        snapshot(&plugin, root.path(), "cli-tool").await["items"],
        json!([])
    );
    assert_eq!(
        snapshot(&plugin, root.path(), &scopes[0]).await["items"][0]["text"],
        "first"
    );
}
