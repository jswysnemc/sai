use super::support::{descriptor, FixtureHost};
use crate::{
    plugins::{operation, registry::register_descriptor},
    tools::ToolRegistry,
};
use serde_json::json;
use std::sync::Arc;

const POLICY: &str = r#"
sai.register_reply_policy({
    prepare=function(input,ctx)
        if not ctx.reply_can_deliver then return {context="previous"} end
        return {context="previous",reminder="prepared:"..input,delivery={input=input}}
    end,
    complete=function(delivery,ctx) return {context="delivered:"..delivery.input} end,
})
"#;

/// 【回复调度测试】【策略目录】创建无工具的独立策略，注册不依赖存在写入工具
/// @param source 策略源码；allowed 为独立授权
/// @returns 真实插件注册表
fn registry(source: &str, allowed: bool) -> ToolRegistry {
    let mut descriptor = descriptor("reply-test", source);
    descriptor.package.manifest.capabilities =
        serde_json::from_value(json!({"reply_policy":true})).unwrap();
    let mut grants = descriptor.package.manifest.capabilities.clone();
    grants.reply_policy = allowed;
    descriptor.setting.grants = Some(grants);
    let mut registry = ToolRegistry::new();
    register_descriptor(
        &mut registry,
        descriptor,
        Arc::new(FixtureHost::default()),
        false,
    )
    .unwrap();
    registry.start_plugin_session("reply-session").unwrap();
    registry
}

/// 【回复调度测试】【完整生命周期】目录只分发获授权策略，完成时消费原实例结果
/// @returns 无；上下文和当前轮提醒各自保留
#[tokio::test]
async fn reply_dispatch_preserves_context_and_completes_once() {
    let registry = registry(POLICY, true);
    operation::scope("reply-op", async {
        let prepared = registry.prepare_plugin_replies("hello", true).await;
        assert!(prepared.has_delivery());
        assert_eq!(prepared.reminder.as_deref(), Some("prepared:hello"));
        assert_eq!(
            registry.current_reply_contexts(&prepared.contexts)["reply-test"],
            "previous"
        );
        let updates = registry.complete_plugin_replies(prepared, true).await;
        assert_eq!(updates["reply-test"].1.as_deref(), Some("delivered:hello"));
    })
    .await;
}

/// 【回复调度测试】【权限与重载】撤权、只读和实例替换均不能消费旧准备结果
/// @returns 无；授权失效后旧上下文也不再可见
#[tokio::test]
async fn reply_dispatch_rejects_readonly_and_replaced_instances() {
    let active = registry(POLICY, true);
    let denied = registry(POLICY, false);
    operation::scope("reply-op", async {
        assert!(denied
            .prepare_plugin_replies("hello", true)
            .await
            .contexts
            .is_empty());
        let readonly = active.prepare_plugin_replies("hello", false).await;
        assert!(!readonly.has_delivery());
        let prepared = active.prepare_plugin_replies("hello", true).await;
        assert!(denied.current_reply_contexts(&prepared.contexts).is_empty());
        assert!(active
            .complete_plugin_replies(prepared, false)
            .await
            .is_empty());
        let prepared = active.prepare_plugin_replies("hello", true).await;
        let replacement = registry(POLICY, true);
        assert!(replacement
            .complete_plugin_replies(prepared, true)
            .await
            .is_empty());
    })
    .await;
}

/// 【回复调度测试】【错误隔离】策略错误只进入诊断，不改变主回复返回类型
/// @returns 无；失败不产生可投递资料
#[tokio::test]
async fn reply_dispatch_isolates_policy_failures() {
    let registry = registry("sai.register_reply_policy({prepare=function() error('broken policy') end,complete=function() end})", true);
    let prepared = registry.prepare_plugin_replies("hello", true).await;
    assert!(prepared.contexts.is_empty());
    assert!(!prepared.has_delivery());
}
