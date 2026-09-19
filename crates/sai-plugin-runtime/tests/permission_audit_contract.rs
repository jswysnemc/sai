#[path = "reply_policy/support.rs"]
mod support;

use sai_plugin_runtime::{
    InvocationContext, PermissionAuditDecision, PermissionAuditInput, PluginManifest,
    PluginPackage, PluginRuntime,
};
use serde_json::json;
use std::sync::Arc;

/// 【审核测试】【实例构造】source 为脚本，declared/granted 为声明和授权；返回真实运行时
fn runtime(source: &str, declared: bool, granted: bool) -> anyhow::Result<PluginRuntime> {
    let manifest = PluginManifest::parse(
        &json!({
            "api_version":1,"id":"audit-fixture","version":"1.0.0","name":"Audit fixture",
            "description":"Permission audit tests","entry":"init.lua",
            "capabilities":{"permission_audit":declared},
            "limits":{"timeout_ms":100,"instructions":10000}
        })
        .to_string(),
    )?;
    let package = PluginPackage::new(manifest, [("init.lua".into(), source.into())].into())?;
    PluginRuntime::load(
        package,
        json!({}),
        serde_json::from_value(json!({"permission_audit":granted}))?,
        Arc::new(support::Host),
    )
}

/// 【审核测试】【输入构造】返回完整审核样本，无参数
fn input() -> PermissionAuditInput {
    PermissionAuditInput {
        tool: "write_file".into(),
        arguments: json!({"path":"a.txt"}),
        context: "create a.txt".into(),
        policy: "follow user scope".into(),
    }
}

/// 超过 Lua 有符号整数范围的参数仍以原始 JSON 数值进入审核，不被浮点舍入
#[tokio::test]
async fn permission_audit_preserves_large_integer_arguments() {
    let plugin = runtime("sai.register_permission_audit({review=function(input) assert(input.arguments_json:find('18446744073709551615',1,true)); return {decision='abstain'} end})", true, true).unwrap();
    let mut facts = input();
    facts.arguments = json!({"id":u64::MAX});
    assert_eq!(
        plugin
            .review_permission(facts, support::context(false))
            .await
            .unwrap()
            .decision,
        PermissionAuditDecision::Abstain
    );
}

/// 独立授权、上下文和只读约束同时生效，未授予能力时不进入 Lua
#[tokio::test]
async fn permission_audit_requires_grant_and_forces_read_only() {
    let source = "sai.register_permission_audit({review=function(input,ctx) assert(not ctx.allow_writes); assert(ctx.session_id=='reply-session'); assert(input.arguments.path=='a.txt'); return {decision='allow',reason='authorized'} end})";
    for (declared, granted) in [(false, false), (true, false), (false, true)] {
        let plugin = runtime(source, declared, granted).unwrap();
        assert!(plugin
            .review_permission(input(), support::context(true))
            .await
            .is_err());
    }
    let plugin = runtime(source, true, true).unwrap();
    assert!(plugin.has_permission_audit());
    assert_eq!(
        plugin
            .review_permission(input(), support::context(true))
            .await
            .unwrap()
            .decision,
        PermissionAuditDecision::Allow
    );
    assert!(plugin
        .review_permission(input(), InvocationContext::default())
        .await
        .is_err());
}

/// 缺失、重复或未知注册字段不能悄悄生成审核入口
#[test]
fn permission_audit_validates_registration() {
    for source in [
        "sai.register_permission_audit({})",
        "sai.register_permission_audit({review=function()end, extra=true})",
        "sai.register_permission_audit({review=function()end}); sai.register_permission_audit({review=function()end})",
    ] {
        assert!(runtime(source,true,true).is_err());
    }
    assert!(!runtime("", true, true).unwrap().has_permission_audit());
}

/// 不合法结果、控制字符、超限输入均不得形成允许决定
#[tokio::test]
async fn permission_audit_rejects_malformed_results_and_incomplete_inputs() {
    for result in [
        "{}",
        "true",
        "{decision='approve'}",
        "{decision='allow',extra=true}",
        "{decision='allow',reason=7}",
        "{decision='allow',reason=string.char(27)}",
        "{decision='allow',reason=string.rep('x',2049)}",
    ] {
        let source =
            format!("sai.register_permission_audit({{review=function() return {result} end}})");
        assert!(
            runtime(&source, true, true)
                .unwrap()
                .review_permission(input(), support::context(false))
                .await
                .is_err(),
            "{result}"
        );
    }
    let plugin = runtime(
        "sai.register_permission_audit({review=function()end})",
        true,
        true,
    )
    .unwrap();
    assert_eq!(
        plugin
            .review_permission(input(), support::context(false))
            .await
            .unwrap()
            .decision,
        PermissionAuditDecision::Abstain
    );
    let mut oversized = input();
    oversized.arguments = json!({"data":"x".repeat(131072)});
    assert!(plugin
        .review_permission(oversized, support::context(false))
        .await
        .is_err());
    let mut invalid = input();
    invalid.arguments = json!([]);
    assert!(plugin
        .review_permission(invalid, support::context(false))
        .await
        .is_err());
}

/// 脚本不能在运行阶段变更注册，无限循环仍受原有预算限制
#[tokio::test]
async fn permission_audit_keeps_execution_budget() {
    for body in [
        "while true do end",
        "sai.register_permission_audit({review=function()end})",
    ] {
        let source = format!("sai.register_permission_audit({{review=function() {body} end}})");
        assert!(runtime(&source, true, true)
            .unwrap()
            .review_permission(input(), support::context(false))
            .await
            .is_err());
    }
}
