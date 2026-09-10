mod common;

use common::notification::{capabilities, runtime, NotificationHost};
use sai_plugin_runtime::{
    Capabilities, EventContext, EventKind, ExecutionLimits, InvocationContext,
};
use serde_json::json;
use std::sync::Arc;

const SOURCE: &str = r#"
    --- 【通知测试】【执行】尝试修改公开上下文，通知权限仍由宿主决定
    --- @param args table 通知请求
    --- @param ctx table 公开调用上下文
    --- @return table 投递结果
    local function execute(args, ctx)
        ctx.allow_writes = true
        ctx.workdir = "/forged"
        return sai.notify.send(args)
    end
    sai.register_tool({name="read",description="read",parameters={type="object"},execute=execute})
    sai.register_tool({name="write",description="write",access="writes",parameters={type="object"},execute=execute})
    sai.register_tool({name="optional",description="optional",access="optional_writes",parameters={type="object"},execute=execute})
    sai.register_command({name="read",description="read",execute=function(args, ctx) return execute(sai.json.decode(args), ctx) end})
    sai.register_command({name="write",description="write",access="writes",execute=function(args, ctx) return execute(sai.json.decode(args), ctx) end})
    sai.on("agent_start",execute)
"#;

/// 【通知接口测试】【独立声明】展示策略授权不能自动获得直接投递能力。
#[test]
fn delivery_requires_a_separate_system_capability() {
    let direct: Capabilities = serde_json::from_value(json!({"system":{"notify":true}})).unwrap();
    let presentation: Capabilities = serde_json::from_value(json!({"notifications":true})).unwrap();
    direct.validate().unwrap();
    assert!(!direct.system.is_empty());
    assert!(!direct.is_subset(&presentation));
    assert!(direct.intersection(&presentation).system.is_empty());
    assert_eq!(direct.intersection(&direct), direct);
    assert!(serde_json::to_value(Capabilities::default()).unwrap()["system"]["notify"].is_null());
}

/// 【通知接口测试】【加载隔离】初始化可以取得接口，但不能发送通知。
#[test]
fn notification_api_does_not_allow_initialization_io() {
    let host = Arc::new(NotificationHost::default());
    runtime(
        r#"
        assert(type(sai.notify) == "table", "notification delivery API is missing")
        local ok, err = pcall(sai.notify.send, {title="initialization"})
        assert(not ok)
    "#,
        capabilities(),
        capabilities(),
        Default::default(),
        host.clone(),
    );
    assert!(host.calls.lock().unwrap().is_empty());
}

/// 【通知接口测试】【纯策略隔离】答复展示实例收窄授权，即使清单声明投递能力也不能执行 I/O。
#[tokio::test]
async fn presentation_callbacks_never_receive_delivery_capabilities() {
    use sai_plugin_runtime::{
        PresentationRuntime, PresentationSurface, ReplyPresentation, ReplyStatus,
    };
    let mut package = common::package(
        r#"
        sai.on("reply_end",function()
            local ok,err=pcall(sai.notify.send,{title="forbidden"})
            assert(not ok and tostring(err):find("notification delivery is not allowed",1,true))
        end)
    "#,
    );
    let mut caps = capabilities();
    caps.notifications = true;
    package.manifest.capabilities = caps.clone();
    let runtime = PresentationRuntime::load(package, json!({}), caps).unwrap();
    assert!(runtime
        .reply_end(&ReplyPresentation {
            surface: PresentationSurface::Tui,
            status: ReplyStatus::Completed,
            locale: "en-US".into()
        })
        .await
        .unwrap()
        .is_empty());
}

/// 【通知接口测试】【结果一致性】宿主不能把未请求的通道报告为成功，错误后下一调用仍可恢复。
#[tokio::test]
async fn inconsistent_host_results_are_rejected() {
    use std::sync::atomic::Ordering;
    let host = Arc::new(NotificationHost::default());
    host.inconsistent.store(true, Ordering::SeqCst);
    let plugin = runtime(
        SOURCE,
        capabilities(),
        capabilities(),
        Default::default(),
        host.clone(),
    );
    let error = plugin
        .call_tool("write", json!({"title":"test"}), writable())
        .await
        .unwrap_err();
    assert!(format!("{error:#}").contains("inconsistent delivery channels"));
    host.inconsistent.store(false, Ordering::SeqCst);
    assert!(plugin
        .call_tool("write", json!({"title":"test"}), writable())
        .await
        .is_ok());
}

/// 【通知接口测试】【授权矩阵】声明、授权及写入权限缺一不可，事件和只读命令不能主动投递。
#[tokio::test]
async fn notification_permissions_are_checked_before_the_host() {
    let presentation: Capabilities = serde_json::from_value(json!({"notifications":true})).unwrap();
    for (declared, granted) in [
        (capabilities(), Capabilities::default()),
        (Capabilities::default(), capabilities()),
        (capabilities(), presentation),
    ] {
        let host = Arc::new(NotificationHost::default());
        let plugin = runtime(SOURCE, declared, granted, Default::default(), host.clone());
        assert!(plugin
            .call_tool("write", json!({"title":"test"}), writable())
            .await
            .is_err());
        assert!(host.calls.lock().unwrap().is_empty());
    }
    let host = Arc::new(NotificationHost::default());
    let plugin = runtime(
        SOURCE,
        capabilities(),
        capabilities(),
        Default::default(),
        host.clone(),
    );
    for tool in ["read", "write", "optional"] {
        for allow_writes in [false, true] {
            let before = host.calls.lock().unwrap().len();
            let context = InvocationContext {
                allow_writes,
                workdir: "/trusted".into(),
                ..Default::default()
            };
            let result = plugin
                .call_tool(tool, json!({"title":"test"}), context)
                .await;
            let allowed = tool != "read" && allow_writes;
            assert_eq!(result.is_ok(), allowed, "{tool}/{allow_writes}: {result:?}");
            let calls = host.calls.lock().unwrap();
            assert_eq!(calls.len(), before + usize::from(allowed));
            if allowed {
                assert_eq!(calls.last().unwrap().1.workdir, "/trusted");
            }
        }
    }
    for command in ["read", "write"] {
        for allow_writes in [false, true] {
            let before = host.calls.lock().unwrap().len();
            let result = plugin
                .call_command(
                    command,
                    r#"{"title":"test"}"#,
                    InvocationContext {
                        allow_writes,
                        ..Default::default()
                    },
                )
                .await;
            let allowed = command == "write" && allow_writes;
            assert_eq!(result.is_ok(), allowed);
            assert_eq!(
                host.calls.lock().unwrap().len(),
                before + usize::from(allowed)
            );
        }
    }
    let before = host.calls.lock().unwrap().len();
    assert!(plugin
        .emit(
            EventKind::AgentStart,
            EventContext {
                data: json!({"title":"event"}),
                ..Default::default()
            }
        )
        .await
        .is_err());
    assert_eq!(host.calls.lock().unwrap().len(), before);
}

/// 【通知接口测试】【输入边界】空通知、过长文本、控制字符、无效声音和超时参数都不能进入宿主。
#[tokio::test]
async fn malformed_requests_do_not_dispatch_or_consume_the_budget() {
    let host = Arc::new(NotificationHost::default());
    let plugin = runtime(
        SOURCE,
        capabilities(),
        capabilities(),
        Default::default(),
        host.clone(),
    );
    for request in [
        json!({}),
        json!({"title":""}),
        json!({"title":"x".repeat(257)}),
        json!({"title":"ok","body":"x".repeat(4097)}),
        json!({"title":"\u{001b}[31m"}),
        json!({"title":"ok","desktop":false}),
        json!({"title":"ok","sound":{"builtin":"unknown"}}),
        json!({"title":"ok","sound":{"builtin":"alarm","path":"x"}}),
        json!({"title":"ok","sound":{"path":"../outside"}}),
        json!({"title":"ok","sound":{"path":"sample.wav"}}),
        json!({"title":"ok","timeout_ms":0}),
        json!({"title":"ok","timeout_ms":120001}),
        json!({"title":"ok","extra":true}),
        json!({"title":true}),
        json!({"title":"ok","desktop":1}),
    ] {
        assert!(
            plugin
                .call_tool("write", request.clone(), writable())
                .await
                .is_err(),
            "{request}"
        );
    }
    assert!(host.calls.lock().unwrap().is_empty());
    let text = plugin
        .call_tool(
            "write",
            json!({"title":"边界","desktop":false,"sound":{"builtin":"alarm"}}),
            writable(),
        )
        .await
        .unwrap();
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&text).unwrap(),
        json!({"desktop":false,"sound":true})
    );
}

/// 【通知接口测试】【调用预算】实际宿主失败仍计费，修改公开额度不能增加本次调用次数。
#[tokio::test]
async fn host_errors_consume_budget_and_subsequent_callbacks_recover() {
    let host = Arc::new(NotificationHost::default());
    let plugin = runtime(
        r#"
        sai.register_tool({name="check",description="check",access="writes",parameters={type="object"},execute=function()
            assert(not pcall(sai.notify.send, {title=""}))
            assert(not pcall(sai.notify.send, {title="fail"}))
            assert(sai.notify.send({title="ok"}).desktop)
            sai.limits.system_calls = 9999
            local ok, err = pcall(sai.notify.send, {title="over-budget"})
            assert(not ok and tostring(err):find("system call budget exceeded",1,true))
            return "ok"
        end})
    "#,
        capabilities(),
        capabilities(),
        ExecutionLimits {
            system_calls: 2,
            ..Default::default()
        },
        host.clone(),
    );
    for count in [2, 4] {
        assert_eq!(
            plugin
                .call_tool("check", json!({}), writable())
                .await
                .unwrap(),
            "ok"
        );
        assert_eq!(host.calls.lock().unwrap().len(), count);
    }
}

/// 【通知接口测试】【缺省宿主】未实现投递的宿主明确报错，不声称发送成功。
#[tokio::test]
async fn unavailable_hosts_are_explicit() {
    let plugin = runtime(
        SOURCE,
        capabilities(),
        capabilities(),
        Default::default(),
        Arc::new(common::RecordingHost::default()),
    );
    let error = plugin
        .call_tool("write", json!({"title":"test"}), writable())
        .await
        .unwrap_err();
    assert!(format!("{error:#}").contains("notification delivery is unavailable"));
}

/// 【通知接口测试】【可信权限】创建明确允许写入的宿主上下文。
/// @returns 写入调用上下文
fn writable() -> InvocationContext {
    InvocationContext {
        allow_writes: true,
        ..Default::default()
    }
}
