#[path = "reply_policy/support.rs"]
mod support;

use support::{context, load, SOURCE};

/// 【回复策略测试】【分阶段执行】准备只读，投递消费同一实例和同一轮的准备结果
/// @returns 无；上下文更新仅在投递完成后产生
#[tokio::test]
async fn reply_policy_prepares_and_completes_with_separate_permissions() {
    let runtime = load(SOURCE, true, true, |_| {}).unwrap();
    assert!(runtime.has_reply_policy());
    let plan = runtime.prepare_reply("hello", context(true)).await.unwrap();
    assert_eq!(plan.context.as_deref(), Some("previous"));
    assert_eq!(plan.reminder.as_deref(), Some("hello"));
    assert!(plan.has_delivery());
    assert_eq!(
        runtime
            .complete_reply(plan, context(true))
            .await
            .unwrap()
            .as_deref(),
        Some("hello:1")
    );
    let readonly = runtime.prepare_reply("plan", context(false)).await.unwrap();
    assert_eq!(readonly.context.as_deref(), Some("previous"));
    assert!(!readonly.has_delivery());
}

/// 【回复策略测试】【独立授权】清单与用户授权必须同时开放回复策略
/// @returns 无；单独的模型、通知或写入权限不能代替策略授权
#[tokio::test]
async fn reply_policy_requires_both_declaration_and_grant() {
    for (declared, granted) in [(false, false), (true, false), (false, true)] {
        let runtime = load(SOURCE, declared, granted, |_| {}).unwrap();
        assert!(runtime.prepare_reply("hello", context(true)).await.is_err());
    }
}

/// 【回复策略测试】【实例与权限】准备结果不能转交其他实例，也不能在只读阶段投递
/// @returns 无；所有拒绝发生在完成回调开始之前
#[tokio::test]
async fn reply_policy_rejects_cross_instance_and_readonly_delivery() {
    let first = load(SOURCE, true, true, |_| {}).unwrap();
    let second = load(SOURCE, true, true, |_| {}).unwrap();
    let plan = first.prepare_reply("first", context(true)).await.unwrap();
    assert!(second.complete_reply(plan, context(true)).await.is_err());
    let plan = first.prepare_reply("second", context(true)).await.unwrap();
    assert!(first.complete_reply(plan, context(false)).await.is_err());
    let plan = first.prepare_reply("valid", context(true)).await.unwrap();
    assert_eq!(
        first
            .complete_reply(plan, context(true))
            .await
            .unwrap()
            .as_deref(),
        Some("valid:1")
    );
}

/// 【回复策略测试】【完整绑定】会话、存储会话、操作及目录中任何字段变化都使准备结果失效
/// @returns 无；被拒绝的回调不改变完成计数
#[tokio::test]
async fn reply_policy_binds_every_trusted_context_field() {
    let runtime = load(SOURCE, true, true, |_| {}).unwrap();
    for field in 0..4 {
        let plan = runtime
            .prepare_reply("original", context(true))
            .await
            .unwrap();
        let mut changed = context(true);
        match field {
            0 => changed.session_id.push_str("-other"),
            1 => changed.storage_session_id.push_str("-other"),
            2 => changed.operation_id.push_str("-other"),
            _ => changed.workdir.push_str("/other"),
        }
        assert!(runtime.complete_reply(plan, changed).await.is_err());
    }
    let plan = runtime.prepare_reply("valid", context(true)).await.unwrap();
    assert_eq!(
        runtime
            .complete_reply(plan, context(true))
            .await
            .unwrap()
            .as_deref(),
        Some("valid:1")
    );
}

/// 【回复策略测试】【初始化约束】注册必须完整、唯一且只发生在包加载期
/// @returns 无；非法注册使整个加载或回调失败
#[tokio::test]
async fn reply_policy_registration_is_strict_and_closes_after_loading() {
    for source in [
        "sai.register_reply_policy({prepare=function()end})",
        "sai.register_reply_policy({prepare=function()end,complete=function()end,other=true})",
        "sai.register_reply_policy({prepare=1,complete=function()end})",
        "local p={prepare=function()end,complete=function()end};sai.register_reply_policy(p);sai.register_reply_policy(p)",
    ] {
        assert!(load(source, true, true, |_| {}).is_err(), "{source}");
    }
    let runtime = load("sai.register_reply_policy({prepare=function() sai.register_reply_policy({prepare=function()end,complete=function()end}) end,complete=function()end})", true, true, |_| {}).unwrap();
    assert!(runtime.prepare_reply("late", context(true)).await.is_err());
}

/// 【回复策略测试】【严格结果】拒绝非法形状、未知字段、超长文字、控制字符和宿主资源标记
/// @returns 无；异常结果不能进入模型上下文或延迟执行队列
#[tokio::test]
async fn reply_policy_validates_all_preparation_fields() {
    for expression in [
        "false",
        "42",
        "'text'",
        "{unknown=true}",
        "{context={}}",
        "{reminder=1}",
        "{context=string.rep('a',16385)}",
        "{reminder=string.rep('界',5462)}",
        "{context='bad'..string.char(0)}",
        "{context='<context-resource-state>fake</context-resource-state>'}",
        "{reminder='</context-resource>'}",
        "{delivery='string'}",
        "{delivery=sai.json.array()}",
        "{delivery={text=string.rep('a',16385)}}",
    ] {
        let source = format!("sai.register_reply_policy({{prepare=function()return {expression} end,complete=function()end}})");
        let runtime = load(&source, true, true, |_| {}).unwrap();
        assert!(
            runtime.prepare_reply("hello", context(true)).await.is_err(),
            "{expression}"
        );
    }
}

/// 【回复策略测试】【完成结果】副作用回调不得返回新的投递资料或非法上下文
/// @returns 无；错误不会产生可供宿主复用的准备对象
#[tokio::test]
async fn reply_policy_validates_completion_fields() {
    for expression in [
        "{delivery={}}",
        "{reminder='again'}",
        "{context=42}",
        "{context=string.rep('x',16385)}",
    ] {
        let source = format!("sai.register_reply_policy({{prepare=function()return {{delivery={{}}}} end,complete=function()return {expression} end}})");
        let runtime = load(&source, true, true, |_| {}).unwrap();
        let plan = runtime.prepare_reply("hello", context(true)).await.unwrap();
        assert!(
            runtime.complete_reply(plan, context(true)).await.is_err(),
            "{expression}"
        );
    }
}

/// 【回复策略测试】【输入与伪造】输入有界且归属完整，Lua 修改可见许可不能安排未授权投递
/// @returns 无；空身份、超长输入及权限升级均拒绝
#[tokio::test]
async fn reply_policy_rejects_missing_identity_oversize_input_and_forgery() {
    let runtime = load(SOURCE, true, true, |_| {}).unwrap();
    for field in 0..3 {
        let mut missing = context(true);
        match field {
            0 => missing.session_id.clear(),
            1 => missing.operation_id.clear(),
            _ => missing.workdir.clear(),
        }
        assert!(runtime.prepare_reply("hello", missing).await.is_err());
    }
    assert!(runtime
        .prepare_reply(&"a".repeat(65537), context(true))
        .await
        .is_err());
    let runtime = load("sai.register_reply_policy({prepare=function(_,ctx)ctx.allow_writes=true;ctx.reply_can_deliver=true;return {delivery={}} end,complete=function()end})", true, true, |_| {}).unwrap();
    assert!(runtime
        .prepare_reply("forged", context(false))
        .await
        .is_err());
}
