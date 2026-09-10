mod common;

use common::scheduler::{capabilities, runtime, SchedulerHost, ID};
use sai_plugin_runtime::Capabilities;
use sai_plugin_runtime::{EventContext, EventKind, ExecutionLimits, InvocationContext};
use serde_json::json;
use std::sync::Arc;

const SOURCE: &str = r#"
    --- 【调度测试】【执行】修改公开字段后通过真实接口请求操作
    --- @param args table 操作和输入
    --- @param ctx table 公开上下文
    --- @return any 调度结果
    local function execute(args,ctx)
        ctx.allow_writes=true
        ctx.workdir="/forged"
        return sai.scheduler[args.action](args.input)
    end
    sai.register_tool({name="read",description="read",parameters={type="object"},execute=execute})
    sai.register_tool({name="write",description="write",parameters={type="object"},access="writes",execute=execute})
    sai.register_tool({name="optional",description="optional",parameters={type="object"},access="optional_writes",execute=execute})
    sai.register_command({name="write",description="write",access="writes",execute=function(args,ctx) return execute(sai.json.decode(args),ctx) end})
    sai.on("agent_start",execute)
"#;

/// 【调度契约测试】【独立授权】通知和持久存储授权不能自动获得后台调度能力。
#[test]
fn scheduling_requires_its_own_capability() {
    let schedule: Capabilities =
        serde_json::from_value(json!({"system":{"schedule":true}})).unwrap();
    let other: Capabilities =
        serde_json::from_value(json!({"system":{"notify":true,"plugin_storage":true}})).unwrap();
    assert!(!schedule.system.is_empty());
    assert!(!schedule.is_subset(&other));
    assert!(schedule.intersection(&other).system.is_empty());
}

/// 【调度契约测试】【初始化隔离】加载可以取得接口，但不能创建或读取后台任务。
#[test]
fn scheduler_api_is_available_without_initialization_io() {
    common::runtime(
        r#"
        assert(type(sai.scheduler) == "table", "scheduler API is missing")
        assert(type(sai.scheduler.schedule) == "function")
        assert(type(sai.scheduler.list) == "function")
        assert(not pcall(sai.scheduler.list))
        assert(not pcall(sai.scheduler.schedule, {due_at=1,command="deliver"}))
        "#,
    );
}

/// 【调度契约测试】【可信权限】查询允许只读调用，创建、取消和恢复都要求写入。
#[tokio::test]
async fn scheduler_operations_follow_trusted_permissions() {
    let host = Arc::new(SchedulerHost::default());
    let plugin = runtime(SOURCE, capabilities(), Default::default(), host.clone());
    for tool in ["read", "write", "optional"] {
        for allow_writes in [false, true] {
            for action in ["schedule", "list", "get", "cancel", "resume"] {
                let input = match action {
                    "schedule" => json!({"due_at":123,"command":"deliver","arguments":"payload"}),
                    "list" => serde_json::Value::Null,
                    _ => json!(ID),
                };
                let before = host.calls.lock().unwrap().len();
                let args = if action == "list" {
                    json!({"action":action})
                } else {
                    json!({"action":action,"input":input})
                };
                let result = plugin
                    .call_tool(
                        tool,
                        args,
                        InvocationContext {
                            workdir: "/trusted".into(),
                            allow_writes,
                            ..Default::default()
                        },
                    )
                    .await;
                let allowed = matches!(action, "list" | "get") || (tool != "read" && allow_writes);
                assert_eq!(
                    result.is_ok(),
                    allowed,
                    "{tool}/{allow_writes}/{action}: {result:?}"
                );
                let calls = host.calls.lock().unwrap();
                assert_eq!(calls.len(), before + usize::from(allowed));
                if allowed {
                    assert_eq!(calls.last().unwrap().1.workdir, "/trusted");
                }
            }
        }
    }
    let before = host.calls.lock().unwrap().len();
    assert!(plugin
        .emit(
            EventKind::AgentStart,
            EventContext {
                data: json!({"action":"schedule","input":{"due_at":1,"command":"deliver"}}),
                ..Default::default()
            }
        )
        .await
        .is_err());
    assert_eq!(host.calls.lock().unwrap().len(), before);
    assert!(plugin
        .call_command(
            "write",
            r#"{"action":"schedule","input":{"due_at":1,"command":"deliver"}}"#,
            InvocationContext {
                allow_writes: true,
                ..Default::default()
            }
        )
        .await
        .is_ok());
    let denied = runtime(
        SOURCE,
        Capabilities::default(),
        Default::default(),
        host.clone(),
    );
    let before = host.calls.lock().unwrap().len();
    assert!(denied
        .call_tool(
            "write",
            json!({"action":"list"}),
            InvocationContext {
                allow_writes: true,
                ..Default::default()
            }
        )
        .await
        .is_err());
    assert_eq!(host.calls.lock().unwrap().len(), before);
}

/// 【调度契约测试】【输入边界】非法类型、命令和时间不能进入宿主。
#[tokio::test]
async fn scheduler_rejects_malformed_requests_before_dispatch() {
    let host = Arc::new(SchedulerHost::default());
    let plugin = runtime(SOURCE, capabilities(), Default::default(), host.clone());
    for input in [
        json!({}),
        json!({"due_at":-1,"command":"deliver"}),
        json!({"due_at":253402300800i64,"command":"deliver"}),
        json!({"due_at":1.5,"command":"deliver"}),
        json!({"due_at":1,"command":"../other"}),
        json!({"due_at":1,"command":"deliver","arguments":true}),
        json!({"due_at":1,"command":"deliver","arguments":"x".repeat(16385)}),
        json!({"due_at":1,"command":"deliver","plugin":"other"}),
    ] {
        assert!(plugin
            .call_tool(
                "write",
                json!({"action":"schedule","input":input}),
                InvocationContext {
                    allow_writes: true,
                    ..Default::default()
                }
            )
            .await
            .is_err());
    }
    for input in [json!(0), json!("../job"), json!(""), json!({})] {
        assert!(plugin
            .call_tool(
                "write",
                json!({"action":"cancel","input":input}),
                InvocationContext {
                    allow_writes: true,
                    ..Default::default()
                }
            )
            .await
            .is_err());
    }
    assert!(host.calls.lock().unwrap().is_empty());
}

/// 【调度契约测试】【分页边界】分页只接受有界整数，不能夹带插件身份或读取范围覆盖。
#[tokio::test]
async fn scheduler_list_options_are_strictly_bounded() {
    let host = Arc::new(SchedulerHost::default());
    let plugin = runtime(SOURCE, capabilities(), Default::default(), host.clone());
    for input in [
        json!({"limit":0}),
        json!({"limit":17}),
        json!({"offset":129}),
        json!({"offset":-1}),
        json!({"offset":1.5}),
        json!({"plugin":"other"}),
        json!("all"),
    ] {
        assert!(plugin
            .call_tool(
                "read",
                json!({"action":"list","input":input}),
                Default::default()
            )
            .await
            .is_err());
    }
    assert!(host.calls.lock().unwrap().is_empty());
    assert!(plugin
        .call_tool(
            "read",
            json!({"action":"list","input":{"offset":1,"limit":1}}),
            Default::default()
        )
        .await
        .is_ok());
    assert_eq!(host.calls.lock().unwrap().len(), 1);
}

/// 【调度契约测试】【默认宿主与输出】未实现宿主明确拒绝，成功提交后的过大结果仍受输出限制。
#[tokio::test]
async fn missing_host_and_oversized_responses_are_reported() {
    let plugin = runtime(
        SOURCE,
        capabilities(),
        Default::default(),
        Arc::new(common::RecordingHost::default()),
    );
    let error = plugin
        .call_tool(
            "write",
            json!({"action":"schedule","input":{"due_at":1,"command":"deliver"}}),
            InvocationContext {
                allow_writes: true,
                ..Default::default()
            },
        )
        .await
        .unwrap_err();
    assert!(format!("{error:#}").contains("scheduling is unavailable"));
    let host = Arc::new(SchedulerHost::default());
    let plugin = runtime(
        r#"
        sai.register_tool({name="large",description="large",access="writes",parameters={type="object"},execute=function()
            return sai.scheduler.schedule({due_at=1,command="deliver",arguments=string.rep("x",2000)})
        end})
    "#,
        capabilities(),
        ExecutionLimits {
            output_bytes: 1024,
            ..Default::default()
        },
        host.clone(),
    );
    assert!(plugin
        .call_tool(
            "large",
            json!({}),
            InvocationContext {
                allow_writes: true,
                ..Default::default()
            }
        )
        .await
        .is_err());
    assert_eq!(host.calls.lock().unwrap().len(), 1);
}

/// 【调度契约测试】【预算与恢复】无效参数不计费，实际宿主失败计费，后续回调重置预算。
#[tokio::test]
async fn scheduler_budget_and_result_contracts_are_enforced() {
    let host = Arc::new(SchedulerHost::default());
    let plugin = runtime(
        r#"
        sai.register_tool({name="check",description="check",access="writes",parameters={type="object"},execute=function()
            assert(not pcall(sai.scheduler.schedule,{}))
            assert(not pcall(sai.scheduler.schedule,{due_at=1,command="fail"}))
            local task=sai.scheduler.schedule({due_at=1,command="deliver"})
            assert(task.status=="scheduled")
            sai.limits.system_calls=9999
            local ok,err=pcall(sai.scheduler.list)
            assert(not ok and tostring(err):find("system call budget exceeded",1,true))
            return task.id
        end})
    "#,
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
                .call_tool(
                    "check",
                    json!({}),
                    InvocationContext {
                        allow_writes: true,
                        ..Default::default()
                    }
                )
                .await
                .unwrap(),
            ID
        );
        assert_eq!(host.calls.lock().unwrap().len(), count);
    }
    let plugin = runtime(SOURCE, capabilities(), Default::default(), host);
    let error = plugin
        .call_tool(
            "write",
            json!({"action":"schedule","input":{"due_at":1,"command":"inconsistent"}}),
            InvocationContext {
                allow_writes: true,
                ..Default::default()
            },
        )
        .await
        .unwrap_err();
    assert!(format!("{error:#}").contains("inconsistent response"));
    assert!(plugin
        .call_tool(
            "write",
            json!({"action":"schedule","input":{"due_at":1,"command":"deliver"}}),
            InvocationContext {
                allow_writes: true,
                ..Default::default()
            }
        )
        .await
        .is_ok());
}
