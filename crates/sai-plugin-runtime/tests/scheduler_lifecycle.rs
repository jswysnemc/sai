mod common;

use common::scheduler::{capabilities, runtime, SchedulerHost};
use sai_plugin_runtime::{ExecutionLimits, InvocationContext};
use serde_json::json;
use std::sync::{atomic::Ordering, Arc};
use std::time::Duration;

/// 【调度生命周期测试】【调用取消】回调超时与主动取消释放宿主 Future，下一次调用可以恢复。
#[tokio::test]
async fn scheduler_cancellation_and_timeout_release_host_operations() {
    for cancel in [false, true] {
        let host = Arc::new(SchedulerHost::default());
        host.pending.store(true, Ordering::SeqCst);
        let plugin = runtime(
            r#"
            sai.register_tool({name="create",description="create",access="writes",parameters={type="object"},execute=function()
                return sai.scheduler.schedule({due_at=1,command="deliver"})
            end})
        "#,
            capabilities(),
            ExecutionLimits {
                timeout_ms: if cancel { 2000 } else { 100 },
                ..Default::default()
            },
            host.clone(),
        );
        let copy = plugin.clone();
        let task = tokio::spawn(async move {
            copy.call_tool(
                "create",
                json!({}),
                InvocationContext {
                    allow_writes: true,
                    ..Default::default()
                },
            )
            .await
        });
        tokio::time::timeout(Duration::from_secs(2), host.entered.notified())
            .await
            .unwrap();
        if cancel {
            task.abort();
            assert!(task.await.unwrap_err().is_cancelled());
        } else {
            assert!(format!("{:#}", task.await.unwrap().unwrap_err()).contains("timed out"));
        }
        tokio::time::timeout(Duration::from_secs(2), host.released.notified())
            .await
            .unwrap();
        assert_eq!(host.active.load(Ordering::SeqCst), 0);
        host.pending.store(false, Ordering::SeqCst);
        assert!(plugin
            .call_tool(
                "create",
                json!({}),
                InvocationContext {
                    allow_writes: true,
                    ..Default::default()
                }
            )
            .await
            .is_ok());
    }
}

/// 【调度生命周期测试】【纯展示隔离】纯通知回调不能继承调度授权或创建后台任务。
#[tokio::test]
async fn presentation_callbacks_cannot_schedule_background_work() {
    use sai_plugin_runtime::{
        PresentationRuntime, PresentationSurface, ReplyPresentation, ReplyStatus,
    };
    let mut package = common::package(
        r#"
        sai.on("reply_end",function()
            local ok,err=pcall(sai.scheduler.schedule,{due_at=1,command="deliver"})
            assert(not ok and tostring(err):find("scheduling is not allowed",1,true))
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
