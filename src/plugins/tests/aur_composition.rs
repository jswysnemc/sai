use super::aur_support::{self, AurHost};
use crate::plugins::{
    config::PluginSetting,
    discovery::{PluginDescriptor, PluginSource},
    registry::register_descriptor,
};
use crate::{paths::SaiPaths, tools::ToolRegistry};
use serde_json::json;
use std::sync::Arc;

/// 【AUR 组合测试】【工具目录】把正式 AUR 包与明确授权的调用插件放入同一会话。
/// @param host 假进程宿主
/// @returns 同时支持直接与组合调用的注册表
fn registry(host: Arc<AurHost>) -> ToolRegistry {
    let mut registry = ToolRegistry::new();
    register_descriptor(
        &mut registry,
        PluginDescriptor {
            package: aur_support::package(),
            source: PluginSource::Bundled,
            setting: PluginSetting {
                enabled: true,
                ..Default::default()
            },
            overrides: None,
        },
        host.clone(),
        false,
    )
    .unwrap();
    let mut wrapper = super::support::descriptor(
        "delegator",
        r#"
        sai.register_tool({name="workflow",description="workflow",access="writes",parameters={type="object"},execute=function(args)
            if args.action ~= "install" then sai.tools.call("review_aur_package", {package="demo"}) end
            if args.action ~= "review" then return sai.tools.call("install_aur_package", {package="demo",user_confirmed=true}) end
            return "reviewed"
        end})
    "#,
    );
    wrapper.package.manifest.capabilities =
        serde_json::from_value(json!({"tools":["review_aur_package","install_aur_package"]}))
            .unwrap();
    wrapper.setting.grants = Some(wrapper.package.manifest.capabilities.clone());
    register_descriptor(&mut registry, wrapper, host, false).unwrap();
    registry.start_plugin_session("conversation").unwrap();
    registry
}

/// 【AUR 组合测试】【分轮约束】Lua 组合调用不能用新 VM 或子调用标识绕过同一用户操作检查。
#[tokio::test]
async fn aur_nested_calls_share_the_real_user_operation_and_review_scope() {
    let root = tempfile::tempdir().unwrap();
    let host = Arc::new(AurHost::new(
        &SaiPaths::for_tests(root.path()),
        Some("paru"),
        "safe",
    ));
    let registry = registry(host.clone());
    let error = registry
        .call("lua__delegator__workflow", r#"{"action":"both"}"#)
        .await
        .unwrap_err();
    assert!(
        format!("{error:#}").contains("cannot run in the same turn"),
        "{error:#}"
    );
    assert!(!host
        .scenario
        .calls
        .lock()
        .unwrap()
        .iter()
        .any(|call| call.0 == "paru_install"));
    let output = registry
        .call("lua__delegator__workflow", r#"{"action":"install"}"#)
        .await
        .unwrap();
    let output: serde_json::Value = serde_json::from_str(&output).unwrap();
    assert_eq!(output["ok"], true);
    assert_eq!(
        host.scenario
            .calls
            .lock()
            .unwrap()
            .iter()
            .filter(|call| call.0 == "paru_install")
            .count(),
        1
    );
}

/// 【AUR 组合测试】【Agent 操作】同一 Agent 生命周期里的两个直接工具调用也必须等待后续用户操作。
#[tokio::test]
async fn aur_agent_operation_covers_separate_direct_tool_calls() {
    let root = tempfile::tempdir().unwrap();
    let host = Arc::new(AurHost::new(
        &SaiPaths::for_tests(root.path()),
        Some("paru"),
        "safe",
    ));
    let registry = registry(host.clone());
    let events = registry.plugin_events();
    let result: anyhow::Result<String> = events
        .agent_run(json!({"kind":"test"}), async {
            registry
                .call("review_aur_package", r#"{"package":"demo"}"#)
                .await?;
            registry
                .call(
                    "install_aur_package",
                    r#"{"package":"demo","user_confirmed":true}"#,
                )
                .await
        })
        .await;
    assert!(format!("{:#}", result.unwrap_err()).contains("cannot run in the same turn"));
    assert!(!host
        .scenario
        .calls
        .lock()
        .unwrap()
        .iter()
        .any(|call| call.0 == "paru_install"));
}
