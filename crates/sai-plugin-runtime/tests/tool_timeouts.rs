mod common;

use common::services::{context, runtime, RecordingServices};
use sai_plugin_runtime::Capabilities;
use serde_json::{json, Value};
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Duration;

/// 【插件测试】【工具超时】单次工具超时可捕获，宿主 Future 释放后同一实例仍可执行工具。
#[tokio::test]
async fn tool_timeout_is_catchable_and_releases_the_host_future() {
    let grants: Capabilities = serde_json::from_value(json!({"tools":["read"]})).unwrap();
    let plugin = runtime(
        r#"
        sai.register_tool({name="run",description="Tool timeout",parameters={type="object"},execute=function(args)
            if args.recover then
                return sai.tools.call("read", {}, {timeout_ms=1000})
            end
            local ok, err = pcall(sai.tools.call, "read", {}, {timeout_ms=15})
            assert(not ok)
            return tostring(err)
        end})
        "#,
        grants.clone(),
        grants,
        |_| {},
    );
    let services = Arc::new(RecordingServices::default());
    services.block_tool.store(true, Ordering::SeqCst);
    let output = tokio::time::timeout(
        Duration::from_secs(2),
        plugin.call_tool("run", json!({}), context(services.clone(), false)),
    )
    .await
    .expect("tool timeout must finish before the outer test deadline")
    .unwrap();
    assert!(output.contains("tool call timed out"), "{output}");
    assert_eq!(services.active.load(Ordering::SeqCst), 0);
    assert_eq!(services.calls.lock().unwrap().len(), 1);

    services.block_tool.store(false, Ordering::SeqCst);
    assert_eq!(
        plugin
            .call_tool(
                "run",
                json!({"recover":true}),
                context(services.clone(), false)
            )
            .await
            .unwrap(),
        "tool result"
    );
    assert_eq!(services.calls.lock().unwrap().len(), 2);
}

/// 【插件测试】【工具参数】拒绝未知配置与非法时长，校验失败不执行工具或消耗调用额度。
#[tokio::test]
async fn invalid_tool_options_do_not_reach_the_host_or_consume_budget() {
    let grants: Capabilities = serde_json::from_value(json!({"tools":["read"]})).unwrap();
    let plugin = runtime(
        r#"
        sai.register_tool({name="run",description="Tool options",parameters={type="object"},execute=function()
            for _, options in ipairs({
                {timeout_ms=-1}, {timeout_ms=1.5}, {timeout_ms="15"},
                {timeout=15}, {workdir="elsewhere"}, false,
            }) do
                local ok = pcall(sai.tools.call, "read", {}, options)
                assert(not ok)
            end
            return sai.tools.call("read", {}, {})
        end})
        "#,
        grants.clone(),
        grants,
        |limits| limits.tool_calls = 1,
    );
    let services = Arc::new(RecordingServices::default());
    assert_eq!(
        plugin
            .call_tool("run", json!({}), context(services.clone(), false))
            .await
            .unwrap(),
        "tool result"
    );
    assert_eq!(services.calls.lock().unwrap().len(), 1);
}

/// 【插件测试】【超时预算】超时的实际工具调用计入预算，后续超额调用不能再次到达宿主。
#[tokio::test]
async fn timed_out_calls_consume_the_tool_budget() {
    let grants: Capabilities = serde_json::from_value(json!({"tools":["read"]})).unwrap();
    let plugin = runtime(
        r#"
        sai.register_tool({name="run",description="Timeout budget",parameters={type="object"},execute=function()
            local first_ok, first = pcall(sai.tools.call, "read", {}, {timeout_ms=15})
            local second_ok, second = pcall(sai.tools.call, "read", {}, {timeout_ms=15})
            return {first_ok=first_ok,first=tostring(first),second_ok=second_ok,second=tostring(second)}
        end})
        "#,
        grants.clone(),
        grants,
        |limits| limits.tool_calls = 1,
    );
    let services = Arc::new(RecordingServices::default());
    services.block_tool.store(true, Ordering::SeqCst);
    let output = tokio::time::timeout(
        Duration::from_secs(2),
        plugin.call_tool("run", json!({}), context(services.clone(), false)),
    )
    .await
    .expect("tool timeout must finish before the outer test deadline")
    .unwrap();
    let output: Value = serde_json::from_str(&output).unwrap();
    assert_eq!(output["first_ok"], false);
    assert!(output["first"]
        .as_str()
        .unwrap()
        .contains("tool call timed out"));
    assert_eq!(output["second_ok"], false);
    assert!(output["second"]
        .as_str()
        .unwrap()
        .contains("tool call budget exceeded"));
    assert_eq!(services.calls.lock().unwrap().len(), 1);
    assert_eq!(services.active.load(Ordering::SeqCst), 0);
}

/// 【插件测试】【总时长】省略时限或声明极大时限都不能突破回调上限，结束后不保留在途工具。
#[tokio::test]
async fn tool_options_cannot_extend_the_overall_callback_deadline() {
    for duration in [Value::Null, json!(i64::MAX)] {
        let grants: Capabilities = serde_json::from_value(json!({"tools":["read"]})).unwrap();
        let plugin = runtime(
            r#"
            sai.register_tool({name="run",description="Overall timeout",parameters={type="object"},execute=function(args)
                local options = {}
                if type(args.duration) == 'number' then options.timeout_ms = args.duration end
                local ok, err = pcall(sai.tools.call, "read", {}, options)
                assert(not ok)
                return tostring(err)
            end})
            "#,
            grants.clone(),
            grants,
            |limits| limits.timeout_ms = 100,
        );
        let services = Arc::new(RecordingServices::default());
        services.block_tool.store(true, Ordering::SeqCst);
        let result = tokio::time::timeout(
            Duration::from_secs(2),
            plugin.call_tool(
                "run",
                json!({"duration":duration}),
                context(services.clone(), false),
            ),
        )
        .await
        .expect("overall callback deadline must remain bounded");
        let message = match result {
            Ok(output) => output,
            Err(error) => format!("{error:#}"),
        };
        assert!(message.contains("timed out"), "{message}");
        tokio::time::timeout(Duration::from_secs(2), services.released.notified())
            .await
            .unwrap();
        assert_eq!(services.active.load(Ordering::SeqCst), 0);
    }
}
