use super::support::FixtureHost;
use super::xuanxue_support::package;
use crate::config::AppConfig;
use crate::paths::SaiPaths;
use crate::plugins::{self, GrantUpdate};
use sai_plugin_runtime::{Capabilities, InvocationContext, PluginRuntime};
use serde_json::{json, Value};
use std::sync::Arc;

const TOOLS: [&str; 4] = [
    "draw_zhouyi_hexagram",
    "draw_tarot_card",
    "draw_fortune_lot",
    "roll_dice",
];

/// 【玄学迁移测试】【独立启停】四个工具使用普通外部名称并遵守显式开关
/// @returns 无；禁用移除目录项，启用不产生短名称别名或重复工具
#[test]
fn xuanxue_installed_switches_control_namespaced_tools() {
    let root = tempfile::tempdir().unwrap();
    let paths = SaiPaths::for_tests(root.path());
    let config = AppConfig::default();
    super::example_support::install("xuanxue", &paths);
    for enabled in [false, true, false] {
        plugins::set_enabled(&config, &paths, "xuanxue", enabled, GrantUpdate::Keep).unwrap();
        let catalog = crate::tools::tool_catalog(&config, &paths);
        for registry in [
            crate::tools::builtin_registry_without_mcp(&config, &paths),
            crate::tools::readonly_registry(&config, &paths),
        ] {
            assert!(registry.plugin_diagnostics().is_empty());
            for local in TOOLS {
                let name = format!("lua__xuanxue__{local}");
                assert!(!registry.contains(local));
                assert_eq!(registry.contains(&name), enabled);
                assert_eq!(
                    catalog.iter().filter(|entry| entry.name == name).count(),
                    usize::from(enabled)
                );
                if enabled {
                    let filtered = registry.clone_filtered(&[&name]);
                    assert_eq!(filtered.definitions().len(), 1);
                    assert_eq!(filtered.plugin_owner(&name), Some("xuanxue"));
                }
            }
        }
    }
}

/// 【玄学迁移测试】【零外部权限】真实清单没有外部能力，过量宿主授权也不能开放 I/O
/// @returns 无；所有业务工具成功，网络、文件、环境、进程、通知、调度、存储和服务均拒绝
#[tokio::test]
async fn xuanxue_has_no_external_capabilities_and_cannot_acquire_them() {
    let package = package(
        "",
        r#"
        --- 【玄学迁移测试】【越权探测】用实际包的空声明检查各类外部访问
        --- @param args table 空参数
        --- @param ctx table 当前回调上下文
        --- @return integer 成功拒绝的调用数量
        local function forbidden(args, ctx)
            local probes = {
                {sai.http.request, {url="https://example.test"}},
                {sai.fs.read_text, "file"},
                {sai.env.get, "PATH"},
                {sai.process.output, "read", {}},
                {sai.notify.send, {title="probe"}},
                {sai.scheduler.list},
                {sai.storage.plugin.get, "key"},
                {sai.tools.call, "read_file", {}},
                {sai.model.complete, {messages={{role="user",content="probe"}}}},
            }
            for _, probe in ipairs(probes) do
                local ok, message = pcall(table.unpack(probe))
                assert(not ok and tostring(message):find("not allowed"), tostring(message))
            end
            return #probes
        end
        sai.register_tool({name="forbidden",description="Capability probe",access="writes",parameters={type="object"},execute=forbidden})
    "#,
    );
    let manifest: Value = serde_json::from_str(include_str!(
        "../../../examples/lua-plugins/xuanxue/sai-plugin.json"
    ))
    .unwrap();
    assert_eq!(manifest["capabilities"], json!({}));
    assert_eq!(package.manifest.capabilities, Capabilities::default());
    let grants: Capabilities = serde_json::from_value(json!({
        "http":["https://example.test"], "model":true, "tools":["read_file"],
        "system":{"read_paths":["."],"environment":["PATH"],"notify":true,"schedule":true,"plugin_storage":true},
    })).unwrap();
    let host = Arc::new(FixtureHost::default());
    let plugin = PluginRuntime::load(package, json!({}), grants, host.clone()).unwrap();
    let root = tempfile::tempdir().unwrap();
    for allow_writes in [false, true] {
        let context = InvocationContext {
            allow_writes,
            workdir: root.path().display().to_string(),
            ..Default::default()
        };
        assert_eq!(
            plugin
                .call_tool("forbidden", json!({}), context.clone())
                .await
                .unwrap(),
            "9"
        );
        for tool in TOOLS {
            assert!(!plugin
                .call_tool(tool, json!({}), context.clone())
                .await
                .unwrap()
                .is_empty());
        }
    }
    assert!(host.requests.lock().unwrap().is_empty());
}
