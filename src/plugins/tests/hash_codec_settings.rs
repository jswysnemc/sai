use super::support::FixtureHost;
use crate::config::AppConfig;
use crate::paths::SaiPaths;
use crate::plugins::{self, GrantUpdate};
use sai_plugin_runtime::{Capabilities, InvocationContext, PluginPackage, PluginRuntime};
use serde_json::{json, Value};
use std::sync::Arc;

const TOOLS: [&str; 2] = ["calculate_hash", "decode_encoded_text"];

/// 【哈希设置测试】【开关兼容】旧开关提供默认值，显式插件开关覆盖普通与只读注册入口
/// @returns 无；启用后工具归属于 Lua 包，过滤后的 Agent 目录保留该归属
#[test]
fn hash_codec_plugin_switches_override_legacy_defaults() {
    let root = tempfile::tempdir().unwrap();
    let paths = SaiPaths::for_tests(root.path());
    let mut config = AppConfig::default();
    for enabled in [true, false] {
        config.plugins.hash_codec.enabled = enabled;
        for registry in [
            crate::tools::builtin_registry_without_mcp(&config, &paths),
            crate::tools::readonly_registry(&config, &paths),
        ] {
            assert!(registry.plugin_diagnostics().is_empty());
            for tool in TOOLS {
                assert_eq!(registry.contains(tool), enabled);
                if enabled {
                    assert_eq!(registry.plugin_owner(tool), Some("hash-codec"));
                }
            }
        }
    }
    for (enabled, legacy) in [(true, false), (false, true)] {
        config.plugins.hash_codec.enabled = legacy;
        plugins::set_enabled(&config, &paths, "hash-codec", enabled, GrantUpdate::Keep).unwrap();
        let catalog = crate::tools::tool_catalog(&config, &paths);
        for registry in [
            crate::tools::builtin_registry_without_mcp(&config, &paths),
            crate::tools::readonly_registry(&config, &paths),
        ] {
            assert!(registry.plugin_diagnostics().is_empty());
            for tool in TOOLS {
                assert_eq!(registry.contains(tool), enabled);
                assert!(catalog.iter().any(|entry| entry.name == tool));
                if enabled {
                    let filtered = registry.clone_filtered(&[tool]);
                    assert_eq!(filtered.definitions().len(), 1);
                    assert_eq!(filtered.plugin_owner(tool), Some("hash-codec"));
                }
            }
        }
    }
}

/// 【哈希设置测试】【零权限】实际清单显式声明空能力，计算成功且越界网络请求在宿主前拒绝
/// @returns 无；只读与可写上下文都不能替代清单授权
#[tokio::test]
async fn hash_codec_needs_no_external_capabilities_and_cannot_gain_io_access() {
    let package = crate::plugins::bundled::packages()
        .unwrap()
        .into_iter()
        .find(|package| package.manifest.id == "hash-codec")
        .unwrap();
    let manifest: Value =
        serde_json::from_str(include_str!("../../../plugins/hash-codec/sai-plugin.json")).unwrap();
    assert_eq!(manifest["capabilities"], json!({}));
    assert_eq!(
        serde_json::to_value(&package.manifest.capabilities).unwrap(),
        serde_json::to_value(Capabilities::default()).unwrap()
    );
    let mut sources = package.sources().clone();
    sources.get_mut("init.lua").unwrap().push_str(r#"
        --- 【哈希设置测试】【越权探测】使用实际清单验证网络能力无法从上下文或过量授权取得
        --- @return string 不应到达的 HTTP 响应正文
        local function forbidden()
            return sai.http.request({url="https://example.test"}).text
        end
        sai.register_tool({name="forbidden",description="Capability probe",parameters={type="object"},execute=forbidden})
    "#);
    let host = Arc::new(FixtureHost::default());
    let mut grants = Capabilities::default();
    grants.http.insert("https://example.test".into());
    let plugin = PluginRuntime::load(
        PluginPackage::new(package.manifest, sources).unwrap(),
        json!({}),
        grants,
        host.clone(),
    )
    .unwrap();
    for allow_writes in [false, true] {
        let error = plugin
            .call_tool(
                "forbidden",
                json!({}),
                InvocationContext {
                    allow_writes,
                    ..Default::default()
                },
            )
            .await
            .unwrap_err();
        assert!(format!("{error:#}").contains("not allowed"), "{error:#}");
        assert!(plugin
            .call_tool(
                "calculate_hash",
                json!({"input_text":"abc"}),
                InvocationContext {
                    allow_writes,
                    ..Default::default()
                }
            )
            .await
            .is_ok());
    }
    assert!(host.requests.lock().unwrap().is_empty());
}
