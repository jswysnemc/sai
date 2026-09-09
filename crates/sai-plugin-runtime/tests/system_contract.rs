mod common;

use common::system::{capabilities, runtime, SystemHost};
use sai_plugin_runtime::{Capabilities, EventContext, EventKind, InvocationContext};
use serde_json::json;
use std::sync::atomic::Ordering;
use std::sync::Arc;

/// 【系统接口测试】【初始化隔离】加载脚本时不能读取环境、文件或启动进程。
#[test]
fn system_io_is_unavailable_during_initialization() {
    let host = Arc::new(SystemHost::default());
    let _plugin = runtime(
        r#"
        assert(not pcall(sai.env.get, 'LANG'))
        assert(not pcall(sai.fs.stat, 'file'))
        assert(not pcall(sai.fs.read_text, 'file'))
        assert(not pcall(sai.fs.read_dir, '.'))
        assert(not pcall(sai.process.output, 'read', {}))
        assert(type(sai.system.platform) == 'string' and sai.system.process_id > 0)
    "#,
        host.clone(),
        capabilities(),
        |_| {},
    );
    assert!(host.calls.lock().unwrap().is_empty());
}

/// 【系统接口测试】【宿主结果】按请求检查正文、目录条数以及 stdout/stderr，不能仅依赖总输出预算。
#[tokio::test]
async fn host_results_cannot_exceed_per_request_limits() {
    for expression in [
        "sai.fs.read_text('file', {max_bytes=4})",
        "sai.fs.read_dir('.', {max_entries=4})",
        "sai.process.output('read', {}, {max_stdout_bytes=4, max_stderr_bytes=8})",
        "sai.process.output('read', {}, {max_stdout_bytes=8, max_stderr_bytes=4})",
    ] {
        let host = Arc::new(SystemHost::default());
        host.oversized.store(true, Ordering::SeqCst);
        let source = format!("sai.register_tool({{name='run',description='Bounds',parameters={{type='object'}},execute=function() return {expression} end}})");
        let plugin = runtime(&source, host, capabilities(), |_| {});
        let result = plugin
            .call_tool("run", json!({}), InvocationContext::default())
            .await;
        assert!(
            result.is_err(),
            "accepted oversized host result: {expression}"
        );
        assert!(format!("{:#}", result.unwrap_err()).contains("exceeds"));
    }
}

/// 【系统接口测试】【可信上下文】篡改 Lua 工作目录或权限不影响宿主；每轮调用重新绑定目录。
#[tokio::test]
async fn lua_context_mutation_cannot_change_the_host_workdir_or_write_access() {
    let host = Arc::new(SystemHost::default());
    let plugin = runtime(
        r#"
        sai.register_tool({name='run',description='Context',parameters={type='object'},execute=function(_,ctx)
            ctx.workdir='/untrusted'; ctx.allow_writes=true
            assert(not pcall(sai.process.output, 'write', {}))
            sai.fs.stat('file'); sai.process.output('read', {})
            return 'ok'
        end})
    "#,
        host.clone(),
        capabilities(),
        |_| {},
    );
    for workdir in ["/first", "/second"] {
        let context = InvocationContext {
            workdir: workdir.into(),
            allow_writes: true,
            ..Default::default()
        };
        assert_eq!(
            plugin.call_tool("run", json!({}), context).await.unwrap(),
            "ok"
        );
    }
    let calls = host.calls.lock().unwrap();
    assert_eq!(calls.len(), 4);
    for (index, (_, context)) in calls.iter().enumerate() {
        assert_eq!(
            context.workdir,
            if index < 2 { "/first" } else { "/second" }
        );
        assert!(!context.allow_writes);
    }
}

/// 【系统接口测试】【双重写入检查】声明写入的工具仍需宿主允许，事件始终不能执行写入模板。
#[tokio::test]
async fn writing_processes_need_both_tool_and_invocation_permission() {
    let host = Arc::new(SystemHost::default());
    let plugin = runtime(
        r#"
        sai.register_tool({name='run',description='Writing process',access='writes',parameters={type='object'},execute=function()
            sai.process.output('write', {}); return 'ok'
        end})
    "#,
        host.clone(),
        capabilities(),
        |_| {},
    );
    assert!(plugin
        .call_tool("run", json!({}), InvocationContext::default())
        .await
        .is_err());
    assert!(host.calls.lock().unwrap().is_empty());
    let context = InvocationContext {
        allow_writes: true,
        ..Default::default()
    };
    assert_eq!(
        plugin.call_tool("run", json!({}), context).await.unwrap(),
        "ok"
    );
    assert!(host.calls.lock().unwrap()[0].1.allow_writes);
}

/// 【系统接口测试】【参数与预算】错误输入不执行宿主或扣除额度，各系统接口共用本次预算。
#[tokio::test]
async fn invalid_inputs_do_not_consume_budget_and_system_calls_share_a_limit() {
    let host = Arc::new(SystemHost::default());
    let plugin = runtime(
        r#"
        sai.register_tool({name='run',description='Budget',parameters={type='object'},execute=function()
            assert(not pcall(sai.env.get,'SECRET'))
            assert(not pcall(sai.fs.read_text,'../file'))
            assert(not pcall(sai.fs.read_text,'file',{max_bytes=-1}))
            assert(not pcall(sai.fs.read_dir,'.',{max_entries=1.5}))
            assert(not pcall(sai.process.output,'read',{}, {workdir='/escape'}))
            assert(not pcall(sai.process.output,'read',{}, {timeout_ms=false}))
            assert(not pcall(sai.process.output,'unknown',{}))
            assert(sai.env.get('LANG') == 'value')
            assert(sai.fs.stat('missing') == nil)
            local ok,err=pcall(sai.process.output,'read',{})
            assert(not ok and tostring(err):find('system call budget exceeded',1,true))
            return 'ok'
        end})
    "#,
        host.clone(),
        capabilities(),
        |limits| limits.system_calls = 2,
    );
    for _ in 0..2 {
        assert_eq!(
            plugin
                .call_tool("run", json!({}), InvocationContext::default())
                .await
                .unwrap(),
            "ok"
        );
    }
    assert_eq!(host.calls.lock().unwrap().len(), 4);
}

/// 【系统接口测试】【最小授权】未授予的系统能力不能到达宿主。
#[tokio::test]
async fn ungranted_system_calls_are_rejected_before_host_execution() {
    let host = Arc::new(SystemHost::default());
    let plugin = runtime(
        r#"
        sai.register_tool({name='run',description='Denied',parameters={type='object'},execute=function()
            assert(not pcall(sai.env.get,'LANG'))
            assert(not pcall(sai.fs.stat,'file'))
            assert(not pcall(sai.process.output,'read',{}))
            return 'ok'
        end})
    "#,
        host.clone(),
        Capabilities::default(),
        |_| {},
    );
    assert_eq!(
        plugin
            .call_tool("run", json!({}), InvocationContext::default())
            .await
            .unwrap(),
        "ok"
    );
    assert!(host.calls.lock().unwrap().is_empty());
}

/// 【系统接口测试】【事件权限】事件使用新目录和只读权限，不能继承上一写入工具的执行状态。
#[tokio::test]
async fn events_can_read_granted_system_data_but_cannot_inherit_write_access() {
    let host = Arc::new(SystemHost::default());
    let plugin = runtime(
        r#"
        sai.register_tool({name='run',description='Write',access='writes',parameters={type='object'},execute=function()
            sai.process.output('write', {}); return 'ok'
        end})
        sai.on('turn_start',function(_,ctx)
            ctx.workdir='/untrusted'; ctx.allow_writes=true
            assert(not pcall(sai.process.output,'write',{}))
            assert(sai.env.get('LANG') == 'value')
            sai.fs.stat('missing')
        end)
    "#,
        host.clone(),
        capabilities(),
        |_| {},
    );
    plugin
        .call_tool(
            "run",
            json!({}),
            InvocationContext {
                workdir: "/tool".into(),
                allow_writes: true,
                ..Default::default()
            },
        )
        .await
        .unwrap();
    plugin
        .emit(
            EventKind::TurnStart,
            EventContext {
                session_id: "event".into(),
                workdir: "/event".into(),
                data: json!({}),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    let calls = host.calls.lock().unwrap();
    assert_eq!(calls.len(), 3);
    assert!(calls[0].1.allow_writes);
    assert_eq!(calls[2].1.workdir, "/event");
    assert!(!calls[2].1.allow_writes);
}
