use super::alarm_support::{self, AlarmHost};
use sai_plugin_runtime::{InvocationContext, PluginRuntime};
use serde_json::json;
use std::sync::atomic::Ordering;
use std::sync::Arc;

/// 【闹钟投递测试】【通道兼容】只播放一次内置声音或指定文件，不自动打开桌面通知。
/// @returns 无，错误投递保留失败而不伪造成功
#[tokio::test]
async fn alarm_delivery_uses_one_sound_and_preserves_failures() {
    let host = Arc::new(AlarmHost::default());
    let plugin = alarm_support::runtime(host.clone(), None);
    for audio in [
        serde_json::Value::Null,
        json!(alarm_support::audio_path("audio.wav")),
    ] {
        let args = json!({"time":"30s","label":"Reminder","audio_file":audio}).to_string();
        let result = plugin
            .call_command(
                "deliver",
                &args,
                InvocationContext {
                    allow_writes: true,
                    ..Default::default()
                },
            )
            .await
            .unwrap();
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&result).unwrap(),
            json!({"desktop":false,"sound":true})
        );
        assert!(!host.notices.lock().unwrap().last().unwrap().desktop);
    }
    let notices = host.notices.lock().unwrap();
    assert_eq!(notices.len(), 2);
    assert_eq!(
        serde_json::to_value(&notices[0]).unwrap()["sound"],
        json!({"builtin":"alarm"})
    );
    assert_eq!(
        serde_json::to_value(&notices[1]).unwrap()["sound"],
        json!({"path":alarm_support::audio_path("audio.wav")})
    );
    drop(notices);
    host.fail_notice.store(true, Ordering::SeqCst);
    assert!(format!(
        "{:#}",
        plugin
            .call_command(
                "deliver",
                r#"{"time":"1s","label":"bad","audio_file":null}"#,
                InvocationContext {
                    allow_writes: true,
                    ..Default::default()
                }
            )
            .await
            .unwrap_err()
    )
    .contains("fixture audio failed"));
    assert!(!host.active_notice.load(Ordering::SeqCst));
}

/// 【闹钟投递测试】【超时恢复】回调截止时间释放通知 Future，后续合法投递仍可执行。
/// @returns 无
#[tokio::test]
async fn alarm_delivery_timeout_releases_host_and_recovers() {
    let host = Arc::new(AlarmHost::default());
    host.wait_notice.store(true, Ordering::SeqCst);
    let mut package = alarm_support::package();
    package.manifest.limits.timeout_ms = 100;
    let grants = package.manifest.capabilities.clone();
    let plugin = PluginRuntime::load(package, json!({}), grants, host.clone()).unwrap();
    let args = r#"{"time":"1s","label":"wait","audio_file":null}"#;
    let context = InvocationContext {
        allow_writes: true,
        ..Default::default()
    };
    assert!(plugin
        .call_command("deliver", args, context.clone())
        .await
        .is_err());
    tokio::time::timeout(
        std::time::Duration::from_secs(2),
        host.notice_released.notified(),
    )
    .await
    .unwrap();
    assert!(!host.active_notice.load(Ordering::SeqCst));
    host.wait_notice.store(false, Ordering::SeqCst);
    assert!(plugin.call_command("deliver", args, context).await.is_ok());
}
