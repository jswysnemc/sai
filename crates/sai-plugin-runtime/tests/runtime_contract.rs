mod common;

use common::{manifest, package, runtime, RecordingHost};
use sai_plugin_runtime::{
    Capabilities, EventContext, EventKind, InvocationContext, PluginPackage, PluginRuntime,
};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

/// 【插件测试】【状态与事件】验证真实工具、命令和事件共用同一插件状态。
#[tokio::test]
async fn tools_commands_and_events_share_only_their_own_instance() {
    let script = r#"
        local count = 0
        sai.on("turn_end", function(event, ctx) count = count + event.increment end)
        sai.register_command({name="reset", description="重置计数", execute=function(args) count=tonumber(args); return "reset" end})
        sai.register_tool({name="count", description="读取计数", parameters={type="object",properties={},additionalProperties=false}, execute=function(args, ctx)
            count=count+1
            return {count=count, session=ctx.session_id}
        end})
    "#;
    let first = runtime(script);
    let second = runtime(script);
    first
        .emit(
            EventKind::TurnEnd,
            EventContext {
                data: json!({"increment": 4}),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    let context = InvocationContext {
        session_id: "session-a".into(),
        ..Default::default()
    };
    let output: Value = serde_json::from_str(
        &first
            .call_tool("count", json!({}), context.clone())
            .await
            .unwrap(),
    )
    .unwrap();
    assert_eq!(output, json!({"count":5,"session":"session-a"}));
    assert_eq!(
        first.call_command("reset", "9", context).await.unwrap(),
        "reset"
    );
    let output: Value = serde_json::from_str(
        &first
            .call_tool("count", json!({}), InvocationContext::default())
            .await
            .unwrap(),
    )
    .unwrap();
    assert_eq!(output["count"], 10);
    let output: Value = serde_json::from_str(
        &second
            .call_tool("count", json!({}), InvocationContext::default())
            .await
            .unwrap(),
    )
    .unwrap();
    assert_eq!(output["count"], 1);
}

/// 【插件测试】【参数校验】缺失或非法参数不得进入 Lua 业务处理。
#[tokio::test]
async fn schema_validation_precedes_handler_execution() {
    let plugin = runtime(
        r#"
        local calls = 0
        sai.register_tool({name="hello", description="问候", parameters={type="object",properties={name={type="string"}},required={"name"},additionalProperties=false},execute=function(args)
            calls=calls+1
            return args.name .. ":" .. calls
        end})
    "#,
    );
    assert!(plugin
        .call_tool("hello", json!({}), InvocationContext::default())
        .await
        .is_err());
    assert!(plugin
        .call_tool("hello", json!({"name": 42}), InvocationContext::default())
        .await
        .is_err());
    assert!(plugin
        .call_tool(
            "hello",
            json!({"name":"Ada","extra":true}),
            InvocationContext::default()
        )
        .await
        .is_err());
    assert_eq!(
        plugin
            .call_tool("hello", json!({"name":"Ada"}), InvocationContext::default())
            .await
            .unwrap(),
        "Ada:1"
    );
}

/// 【插件测试】【网络权限】HTTP 必须同时满足清单声明和宿主授权。
#[tokio::test]
async fn http_capabilities_are_intersected_before_host_dispatch() {
    let source = r#"sai.register_tool({name="fetch",description="读取服务",parameters={type="object",properties={url={type="string"}},required={"url"}},execute=function(args)
        local response=sai.http.request({url=args.url})
        return sai.json.decode(response.text)
    end})"#;
    let mut manifest = manifest();
    manifest
        .capabilities
        .http
        .insert("https://service.test".into());
    let package = PluginPackage::new(
        manifest.clone(),
        BTreeMap::from([("init.lua".into(), source.into())]),
    )
    .unwrap();
    let host = Arc::new(RecordingHost::default());
    let ungranted = PluginRuntime::load(
        package.clone(),
        json!({}),
        Capabilities::default(),
        host.clone(),
    )
    .unwrap();
    assert!(ungranted
        .call_tool(
            "fetch",
            json!({"url":"https://service.test/data"}),
            InvocationContext::default()
        )
        .await
        .is_err());
    assert!(host.requests.lock().unwrap().is_empty());
    let granted =
        PluginRuntime::load(package, json!({}), manifest.capabilities, host.clone()).unwrap();
    assert!(granted
        .call_tool(
            "fetch",
            json!({"url":"https://other.test/data"}),
            InvocationContext::default()
        )
        .await
        .is_err());
    assert!(host.requests.lock().unwrap().is_empty());
    let output = granted
        .call_tool(
            "fetch",
            json!({"url":"https://service.test/data"}),
            InvocationContext::default(),
        )
        .await
        .unwrap();
    assert_eq!(
        serde_json::from_str::<Value>(&output).unwrap(),
        json!({"answer":42})
    );
    assert_eq!(host.requests.lock().unwrap().len(), 1);
}

/// 【插件测试】【写入权限】修改 Lua 参数不能使只读回调发起写入型请求。
#[tokio::test]
async fn writing_http_requires_both_tool_access_and_host_permission() {
    let source = r#"
        local function send(args, ctx) ctx.allow_writes=true; return sai.http.request({url="https://service.test/data",method="POST",body="value"}).text end
        sai.register_tool({name="read",description="只读工具",parameters={type="object"},execute=send})
        sai.register_tool({name="write",description="写入工具",access="writes",parameters={type="object"},execute=send})
    "#;
    let mut manifest = manifest();
    manifest
        .capabilities
        .http
        .insert("https://service.test".into());
    let package = PluginPackage::new(
        manifest.clone(),
        BTreeMap::from([("init.lua".into(), source.into())]),
    )
    .unwrap();
    let host = Arc::new(RecordingHost::default());
    let plugin =
        PluginRuntime::load(package, json!({}), manifest.capabilities, host.clone()).unwrap();
    assert!(plugin
        .call_tool(
            "read",
            json!({}),
            InvocationContext {
                allow_writes: true,
                ..Default::default()
            }
        )
        .await
        .is_err());
    assert!(plugin
        .call_tool("write", json!({}), InvocationContext::default())
        .await
        .is_err());
    assert!(host.requests.lock().unwrap().is_empty());
    plugin
        .call_tool(
            "write",
            json!({}),
            InvocationContext {
                allow_writes: true,
                ..Default::default()
            },
        )
        .await
        .unwrap();
    assert_eq!(host.requests.lock().unwrap().len(), 1);
}

/// 【插件测试】【结果与进度】检查空数组、null、中文和进度信息的真实序列化。
#[tokio::test]
async fn structured_results_preserve_arrays_null_and_progress() {
    let plugin = runtime(
        r#"sai.register_tool({name="result",description="序列化",parameters={type="object"},execute=function(args,ctx)
        ctx.progress("开始处理")
        ctx.progress("处理完成")
        return {items=sai.json.array(),value=sai.json.null,text=sai.text.clip("中文测试",2)}
    end})"#,
    );
    let messages = Arc::new(Mutex::new(Vec::new()));
    let sink = messages.clone();
    let context = InvocationContext {
        progress: Some(Arc::new(move |text| sink.lock().unwrap().push(text))),
        ..Default::default()
    };
    let result: Value = serde_json::from_str(
        &plugin
            .call_tool("result", json!({}), context)
            .await
            .unwrap(),
    )
    .unwrap();
    assert_eq!(
        result,
        json!({"items":[],"value":null,"text":"中文\n...[truncated]"})
    );
    assert_eq!(*messages.lock().unwrap(), vec!["开始处理", "处理完成"]);
}

/// 【插件测试】【模块快照】延迟 require 也只能读取加载时的包内源码。
#[tokio::test]
async fn require_uses_snapshot_and_rejects_paths_and_native_libraries() {
    let root = tempfile::tempdir().unwrap();
    std::fs::write(
        root.path().join("sai-plugin.json"),
        serde_json::to_vec(&manifest()).unwrap(),
    )
    .unwrap();
    std::fs::write(root.path().join("helper.lua"), "return {value='original'}").unwrap();
    std::fs::write(root.path().join("init.lua"),r#"sai.register_tool({name="module",description="加载模块",parameters={type="object",properties={name={type="string"}},required={"name"}},execute=function(args) return require(args.name).value end})"#).unwrap();
    let plugin = PluginRuntime::load(
        PluginPackage::from_directory(root.path()).unwrap(),
        json!({}),
        Capabilities::default(),
        Arc::new(RecordingHost::default()),
    )
    .unwrap();
    std::fs::write(root.path().join("helper.lua"), "return {value='modified'}").unwrap();
    assert_eq!(
        plugin
            .call_tool(
                "module",
                json!({"name":"helper"}),
                InvocationContext::default()
            )
            .await
            .unwrap(),
        "original"
    );
    for name in ["../outside", "/tmp/outside", "C:\\outside", "socket", "os"] {
        assert!(plugin
            .call_tool("module", json!({"name":name}), InvocationContext::default())
            .await
            .is_err());
    }
}

/// 【插件测试】【注册事务】重复注册、外部 Schema 和加载异常都不产生可用实例。
#[test]
fn invalid_registration_fails_the_whole_load() {
    let register = "sai.register_tool({name='ok',description='有效工具',parameters={type='object'},execute=function() return 'ok' end})";
    for source in [format!("{register};{register}"),format!("{register};error('load failed')"),"sai.register_tool({name='bad',description='错误',parameters={type='object',['$ref']='https://example.test/schema'},execute=function() end})".into()] {
        assert!(PluginRuntime::load(package(&source),json!({}),Capabilities::default(),Arc::new(RecordingHost::default())).is_err());
    }
}

/// 【插件测试】【解释器能力】插件不能借助 Lua 标准库访问进程、文件或动态代码。
#[tokio::test]
async fn ambient_system_capabilities_are_absent() {
    let plugin = runtime(
        r#"sai.register_tool({name="check",description="检查能力",parameters={type="object"},execute=function()
        for _, name in ipairs({"io", "os", "package", "debug", "dofile", "loadfile", "load", "coroutine"}) do
            assert(_G[name] == nil, "unexpected global capability: " .. name)
        end
        return "restricted"
    end})"#,
    );
    assert_eq!(
        plugin
            .call_tool("check", json!({}), InvocationContext::default())
            .await
            .unwrap(),
        "restricted"
    );
}

/// 【插件测试】【上下文撤销】保存上一调用的进度函数不能污染旧工具结果通道。
#[tokio::test]
async fn progress_callbacks_expire_when_their_invocation_ends() {
    let plugin = runtime(
        r#"
        local previous
        sai.register_tool({name='check',description='Progress lifetime',parameters={type='object'},execute=function(args,ctx)
            if previous then
                local ok = pcall(previous, 'stale output')
                assert(not ok, 'old progress callback still active')
            end
            ctx.progress('current output')
            previous = ctx.progress
            return 'ok'
        end})
    "#,
    );
    let messages = Arc::new(Mutex::new(Vec::new()));
    let sink = messages.clone();
    let context = InvocationContext {
        progress: Some(Arc::new(move |text| sink.lock().unwrap().push(text))),
        ..Default::default()
    };
    plugin
        .call_tool("check", json!({}), context.clone())
        .await
        .unwrap();
    plugin.call_tool("check", json!({}), context).await.unwrap();
    assert_eq!(
        *messages.lock().unwrap(),
        vec!["current output", "current output"]
    );
}

/// 【插件测试】【HTTP 来源】插件不能用 Host 头改变虚拟主机并绕过来源限制。
#[tokio::test]
async fn transport_headers_cannot_override_the_authorized_origin() {
    let host = Arc::new(RecordingHost::default());
    let mut package = package(
        r#"sai.register_tool({name='request',description='Request',parameters={type='object'},execute=function()
        return sai.http.request({url='https://example.test/',headers={Host='other.test'}})
    end})"#,
    );
    package
        .manifest
        .capabilities
        .http
        .insert("https://example.test".to_string());
    let grants = package.manifest.capabilities.clone();
    let plugin = PluginRuntime::load(package, json!({}), grants, host.clone()).unwrap();
    let error = plugin
        .call_tool("request", json!({}), InvocationContext::default())
        .await
        .unwrap_err();
    assert!(format!("{error:#}").contains("transport headers are host controlled"));
    assert!(host.requests.lock().unwrap().is_empty());
}
