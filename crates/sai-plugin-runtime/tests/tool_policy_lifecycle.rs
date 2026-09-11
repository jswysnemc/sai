#[path = "reply_policy/support.rs"]
mod support;
use sai_plugin_runtime::ToolPolicyInput;
use sai_plugin_runtime::{host::*, Capabilities, PluginManifest, PluginPackage, PluginRuntime};
use serde_json::{json, Value};
use std::{sync::Arc, time::Duration};

/// 【工具策略测试】【输入装配】控制是否停留在循环中，其他字段固定为正常工具事实
/// @param stall 是否暂停
/// @returns 当前工具事实
fn input(stall: bool) -> ToolPolicyInput {
    ToolPolicyInput {
        name: "tool".into(),
        local_name: None,
        arguments: json!({"stall":stall}),
        ok: true,
        tools: vec![],
    }
}

const SOURCE: &str = r#"
sai.register_reply_policy({prepare=function()end,complete=function()end,
    after_tool=function(input,state,ctx)
        if input.arguments.stall then
            ctx.progress("entered")
            while true do end
        end
        return {state={recovered=true}}
    end})
"#;

struct WaitingHost;

#[async_trait::async_trait]
impl PluginHost for WaitingHost {
    /// 【工具策略测试】【受控等待】不发送网络请求，等待外层期限回收请求 Future
    /// @param request 请求；capabilities 为授权；allow_writes 为可信权限
    /// @returns 永不主动完成的请求
    async fn http(&self, _: HttpRequest, _: Capabilities, _: bool) -> anyhow::Result<HttpResponse> {
        std::future::pending().await
    }
}

/// 【工具策略测试】【超时恢复】超时不能锁住实例，之后的有效调用仍可以执行
/// @returns 无；失败调用不返回状态或提醒
#[tokio::test]
async fn tool_policy_timeout_releases_runtime() {
    let source = SOURCE.replace(
        "while true do end",
        "sai.http.request({url='https://fixture.invalid/wait'})",
    );
    let manifest = PluginManifest::parse(
        &json!({
            "api_version":1,"id":"tool-policy-timeout","version":"1.0.0",
            "name":"Tool policy timeout","description":"Callback deadline","entry":"init.lua",
            "capabilities":{"reply_policy":true,"http":["https://fixture.invalid"]},
            "limits":{"timeout_ms":100}
        })
        .to_string(),
    )
    .unwrap();
    let grants = manifest.capabilities.clone();
    let plugin = PluginRuntime::load(
        PluginPackage::new(manifest, [("init.lua".into(), source)].into()).unwrap(),
        json!({}),
        grants,
        Arc::new(WaitingHost),
    )
    .unwrap();
    let error = plugin
        .after_tool(input(true), Value::Null, support::context(false))
        .await
        .err()
        .unwrap();
    assert!(format!("{error:#}").contains("timed out"), "{error:#}");
    assert_eq!(
        plugin
            .after_tool(input(false), Value::Null, support::context(false))
            .await
            .unwrap()
            .state,
        json!({"recovered":true})
    );
}

/// 【工具策略测试】【外部取消】确认回调已经开始再取消，随后原实例必须恢复
/// @returns 无；调用租约和虚拟机锁不会随取消泄漏
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn tool_policy_cancellation_releases_runtime() {
    let plugin = support::load(SOURCE, true, true, |limits| {
        limits.timeout_ms = 10000;
        limits.instructions = 20_000_000;
    })
    .unwrap();
    let started = Arc::new(tokio::sync::Notify::new());
    let signal = started.clone();
    let mut context = support::context(false);
    context.progress = Some(Arc::new(move |_| signal.notify_one()));
    let running = plugin.clone();
    let task =
        tokio::spawn(async move { running.after_tool(input(true), Value::Null, context).await });
    tokio::time::timeout(Duration::from_secs(2), started.notified())
        .await
        .unwrap();
    task.abort();
    assert!(task.await.err().unwrap().is_cancelled());
    let next = tokio::time::timeout(
        Duration::from_secs(2),
        plugin.after_tool(input(false), Value::Null, support::context(false)),
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(next.state, json!({"recovered":true}));
}
