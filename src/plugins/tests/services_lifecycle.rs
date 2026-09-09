use super::services_support::{
    register_service, service_descriptor, tool_call, ModelFixture, ModelReply,
};
use super::support::FixtureHost;
use crate::paths::SaiPaths;
use crate::plugins::registry::register_descriptor;
use crate::tools::{ToolRegistry, ToolSpec};
use serde_json::{json, Value};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Notify;

struct Released(Arc<Notify>);

impl Drop for Released {
    /// 【插件测试】【取消确认】宿主 Future 被释放时通知测试，不等待插件总超时。
    /// @returns 无
    fn drop(&mut self) {
        self.0.notify_one();
    }
}

/// 【插件测试】【取消与继续】取消真实嵌套工具后释放服务和注册表，同一 Lua VM 可以再次调用。
#[tokio::test]
async fn cancellation_releases_native_futures_and_registry_ownership() {
    let entered = Arc::new(Notify::new());
    let released = Arc::new(Notify::new());
    let sentinel = Arc::new(());
    let start = entered.clone();
    let finish = released.clone();
    let capture = sentinel.clone();
    let mut tools = ToolRegistry::new();
    tools.register(ToolSpec::new(
        "wait_leaf",
        "Wait for cancellation",
        json!({"type":"object"}),
        move |args| {
            let start = start.clone();
            let finish = finish.clone();
            let marker = capture.clone();
            async move {
                let _marker = marker;
                if args["block"] == true {
                    let _released = Released(finish);
                    start.notify_one();
                    std::future::pending::<()>().await;
                }
                Ok("resumed".into())
            }
        },
    ));
    register_service(
        &mut tools,
        "cancel",
        r#"
        sai.register_tool({name='run',description='Call cancellable tool',parameters={type='object'},
            execute=function(args) return sai.tools.call('wait_leaf', args) end})
    "#,
        json!({"tools":["wait_leaf"]}),
    );
    let tools = Arc::new(tools);
    let call_tools = tools.clone();
    let task = tokio::spawn(async move {
        call_tools
            .call("lua__cancel__run", r#"{"block":true}"#)
            .await
    });
    tokio::time::timeout(Duration::from_secs(3), entered.notified())
        .await
        .unwrap();
    task.abort();
    assert!(task.await.unwrap_err().is_cancelled());
    tokio::time::timeout(Duration::from_secs(3), released.notified())
        .await
        .unwrap();
    let output = tokio::time::timeout(Duration::from_secs(3), tools.call("lua__cancel__run", "{}"))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(output, "resumed");
    drop(tools);
    assert_eq!(Arc::strong_count(&sentinel), 1);
}

/// 【插件测试】【调用深度】九层不同插件也不能绕过八层上限，必须明确报错而不是等待超时。
#[tokio::test]
async fn deeply_nested_plugins_stop_at_the_host_depth_limit() {
    let mut tools = ToolRegistry::new();
    for index in 0..9 {
        let dependency = format!("lua__layer{}__run", index + 1);
        let source = if index == 8 {
            "sai.register_tool({name='run',description='Unreachable',parameters={type='object'},execute=function() return 'too deep' end})".to_string()
        } else {
            format!("sai.register_tool({{name='run',description='Next layer',parameters={{type='object'}},execute=function() return sai.tools.call('{dependency}', {{}}) end}})")
        };
        register_service(
            &mut tools,
            &format!("layer{index}"),
            &source,
            json!({"tools":[dependency]}),
        );
    }
    let error = tokio::time::timeout(Duration::from_secs(3), tools.call("lua__layer0__run", "{}"))
        .await
        .unwrap()
        .unwrap_err();
    assert!(
        format!("{error:#}").contains("plugin invocation depth exceeds 8"),
        "{error:#}"
    );
}

/// 【插件测试】【流式边界】工具参数超过输出额度时立即取消模型流，不能等待连接结束或请求超时。
#[tokio::test]
async fn oversized_tool_arguments_stop_the_model_stream_before_it_finishes() {
    let fixture = ModelFixture::start(vec![ModelReply::delta(
        json!({"role":"assistant", "tool_calls":[
            tool_call(0, "read_fixture", json!({"text":"x".repeat(32768)}))
        ]}),
    )
    .hold_open()])
    .await;
    let root = tempfile::tempdir().unwrap();
    let paths = SaiPaths::for_tests(root.path());
    let mut tools = ToolRegistry::new();
    tools.set_plugin_model_client(&fixture.client("bounded-model", &paths));
    tools.register(ToolSpec::new(
        "read_fixture",
        "Read fixture",
        json!({"type":"object"}),
        |_| async { panic!("model suggestion executed automatically") },
    ));
    let mut plugin = service_descriptor(
        "bounded",
        r#"
        sai.register_tool({name='run',description='Bound model output',parameters={type='object'},execute=function()
            local ok, result=pcall(sai.model.complete, {messages={{role='user',content='fixture'}},tools={'read_fixture'},timeout_ms=2000})
            return {ok=ok, error=tostring(result)}
        end})
    "#,
        json!({"model":true, "tools":["read_fixture"]}),
    );
    plugin.package.manifest.limits.output_bytes = 1024;
    register_descriptor(&mut tools, plugin, Arc::new(FixtureHost::default()), false).unwrap();
    let output: Value =
        serde_json::from_str(&tools.call("lua__bounded__run", "{}").await.unwrap()).unwrap();
    assert_eq!(output["ok"], false);
    assert!(
        output["error"]
            .as_str()
            .unwrap()
            .contains("plugin model response exceeds size limit"),
        "{output}"
    );
    assert_eq!(fixture.requests().len(), 1);
}
