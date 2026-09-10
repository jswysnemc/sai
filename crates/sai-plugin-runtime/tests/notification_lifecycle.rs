mod common;

use common::notification::{capabilities, runtime, NotificationHost};
use sai_plugin_runtime::{ExecutionLimits, InvocationContext};
use serde_json::json;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Duration;

const SOURCE: &str = r#"
    sai.register_tool({name="send",description="send",access="writes",parameters={type="object"},execute=function(args)
        return sai.notify.send({title="test",timeout_ms=args.timeout})
    end})
"#;

/// 【通知接口测试】【取消与超时】释放等待中的宿主 Future 后，不残留活动投递且可以继续调用。
#[tokio::test]
async fn timeouts_and_cancellation_release_delivery_and_allow_recovery() {
    for scenario in ["request-timeout", "callback-timeout", "cancel"] {
        let host = Arc::new(NotificationHost::default());
        host.pending.store(true, Ordering::SeqCst);
        let plugin = runtime(
            SOURCE,
            capabilities(),
            capabilities(),
            ExecutionLimits {
                timeout_ms: if scenario == "callback-timeout" {
                    100
                } else {
                    2000
                },
                ..Default::default()
            },
            host.clone(),
        );
        let cloned = plugin.clone();
        let timeout = if scenario == "request-timeout" {
            15
        } else {
            120000
        };
        let call = tokio::spawn(async move {
            cloned
                .call_tool(
                    "send",
                    json!({"timeout":timeout}),
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
        if scenario == "cancel" {
            call.abort();
            assert!(call.await.unwrap_err().is_cancelled());
        } else {
            assert!(format!("{:#}", call.await.unwrap().unwrap_err()).contains("timed out"));
        }
        tokio::time::timeout(Duration::from_secs(2), host.released.notified())
            .await
            .unwrap();
        assert_eq!(host.active.load(Ordering::SeqCst), 0);
        host.pending.store(false, Ordering::SeqCst);
        let output = plugin
            .call_tool(
                "send",
                json!({"timeout":1000}),
                InvocationContext {
                    allow_writes: true,
                    ..Default::default()
                },
            )
            .await
            .unwrap();
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&output).unwrap(),
            json!({"desktop":true,"sound":false})
        );
    }
}
