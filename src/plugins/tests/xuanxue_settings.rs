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

/// 【玄学迁移测试】【工具归属】四个公开工具由同一个 Lua 包提供，保留旧开关默认值
/// @returns 无；普通和只读注册表均按有效设置暴露工具
#[test]
fn xuanxue_tools_belong_to_lua_and_follow_legacy_defaults() {
    let root = tempfile::tempdir().unwrap();
    let paths = SaiPaths::for_tests(root.path());
    let mut config = AppConfig::default();
    for enabled in [true, false] {
        config.plugins.xuanxue.enabled = enabled;
        for registry in [
            crate::tools::builtin_registry_without_mcp(&config, &paths),
            crate::tools::readonly_registry(&config, &paths),
        ] {
            assert!(registry.plugin_diagnostics().is_empty());
            for tool in TOOLS {
                assert_eq!(registry.contains(tool), enabled, "{tool}");
                if enabled {
                    assert_eq!(registry.plugin_owner(tool), Some("xuanxue"));
                }
            }
        }
    }
}

/// 【玄学迁移测试】【显式开关】插件设置覆盖旧开关，目录与过滤后的注册表保留工具信息
/// @returns 无；禁用不妨碍配置目录列出工具，启用不会留下重复注册
#[test]
fn xuanxue_explicit_switches_override_defaults_and_preserve_catalog() {
    let root = tempfile::tempdir().unwrap();
    let paths = SaiPaths::for_tests(root.path());
    let mut config = AppConfig::default();
    for enabled in [true, false] {
        config.plugins.xuanxue.enabled = !enabled;
        plugins::set_enabled(&config, &paths, "xuanxue", enabled, GrantUpdate::Keep).unwrap();
        let catalog = crate::tools::tool_catalog(&config, &paths);
        for registry in [
            crate::tools::builtin_registry_without_mcp(&config, &paths),
            crate::tools::readonly_registry(&config, &paths),
        ] {
            assert!(registry.plugin_diagnostics().is_empty());
            for tool in TOOLS {
                assert_eq!(registry.contains(tool), enabled);
                assert_eq!(catalog.iter().filter(|entry| entry.name == tool).count(), 1);
                if enabled {
                    let filtered = registry.clone_filtered(&[tool]);
                    assert_eq!(filtered.definitions().len(), 1);
                    assert_eq!(filtered.plugin_owner(tool), Some("xuanxue"));
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
    let manifest: Value =
        serde_json::from_str(include_str!("../../../plugins/xuanxue/sai-plugin.json")).unwrap();
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
