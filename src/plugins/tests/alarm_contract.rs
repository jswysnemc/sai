use super::alarm_support::{audio_path, call, runtime, task, AlarmHost, NOW};
use sai_plugin_runtime::host::ScheduledStatus;
use sai_plugin_runtime::Capabilities;
use serde_json::{json, Value};
use std::sync::Arc;

/// 【闹钟契约测试】【过渡分页】最多查询 128 条新记录与 128 条旧记录，旧标签保持原样。
/// @returns 无
#[tokio::test]
async fn alarm_lists_full_transition_capacity_and_preserves_legacy_labels() {
    let host = Arc::new(AlarmHost::default());
    for index in 0..256 {
        let payload = json!({"time":"30s","label":"  旧标签\u{0001}  ","audio_file":null,"legacy_id":format!("alarm-1700000000000-{}",index+1)}).to_string();
        host.tasks
            .lock()
            .unwrap()
            .push(task(index, ScheduledStatus::Scheduled, &payload));
    }
    let plugin = runtime(host.clone(), None);
    let result = call(&plugin, "list_alarms", json!({}), false)
        .await
        .unwrap();
    assert_eq!(result["alarms"].as_array().unwrap().len(), 256);
    assert_eq!(result["alarms"][255]["label"], "  旧标签\u{0001}  ");
    host.tasks.lock().unwrap().push(task(
        256,
        ScheduledStatus::Scheduled,
        r#"{"time":"30s","label":"overflow","audio_file":null}"#,
    ));
    assert!(call(&plugin, "list_alarms", json!({}), false)
        .await
        .is_err());
}

/// 【闹钟契约测试】【创建结果】保留旧字段与默认声音，创建只发布任务而不立即投递。
/// @returns 无
#[tokio::test]
async fn alarm_creation_preserves_fields_and_defers_delivery() {
    let host = Arc::new(AlarmHost::default());
    let plugin = runtime(host.clone(), None);
    let result = call(
        &plugin,
        "set_alarm",
        json!({"time":" 1h 2m 3s ","label":"  双语提醒  "}),
        true,
    )
    .await
    .unwrap();
    assert_eq!(
        result,
        json!({"ok":true,"id":"job-00000000000000000000000000000000","time":"1h 2m 3s","label":"双语提醒","audio_file":null,"due_at":NOW+3723,"due_at_local":"2023-11-14 23:15:23","pid":123})
    );
    let tasks = host.tasks.lock().unwrap();
    assert_eq!(tasks.len(), 1);
    assert_eq!(tasks[0].command, "deliver");
    assert_eq!(
        serde_json::from_str::<Value>(&tasks[0].arguments).unwrap(),
        json!({"time":"1h 2m 3s","label":"双语提醒","audio_file":null})
    );
    assert!(host.notices.lock().unwrap().is_empty());
    assert_eq!(host.contexts.lock().unwrap()[0].workdir, audio_path("work"));
}

/// 【闹钟契约测试】【输入边界】无效时间、类型、额外字段和超限文本不能创建任务。
/// @returns 无，失败后仍可创建合法提醒
#[tokio::test]
async fn alarm_rejects_invalid_inputs_before_scheduling() {
    let host = Arc::new(AlarmHost::default());
    let plugin = runtime(host.clone(), None);
    for args in [
        json!({}),
        json!({"time":30}),
        json!({"time":""}),
        json!({"time":"0s"}),
        json!({"time":"1s","other":true}),
        json!({"time":"1s","label":false}),
        json!({"time":"1s","label":"x".repeat(4097)}),
        json!({"time":"1s","label":"bad\u{0000}label"}),
        json!({"time":"1s","label":"bad\u{0080}label"}),
        json!({"time":"18446744073709551615h"}),
        json!({"time":"253402300799s"}),
        json!({"time":"1秒"}),
    ] {
        assert!(
            call(&plugin, "set_alarm", args.clone(), true)
                .await
                .is_err(),
            "{args}"
        );
    }
    assert!(host.tasks.lock().unwrap().is_empty());
    assert!(call(&plugin, "set_alarm", json!({"time":"1s"}), true)
        .await
        .is_ok());
    assert_eq!(host.tasks.lock().unwrap().len(), 1);
}

/// 【闹钟契约测试】【权限分离】调度和投递分别授权，只读调用只能查询。
/// @returns 无
#[tokio::test]
async fn alarm_permissions_remain_independent() {
    let host = Arc::new(AlarmHost::default());
    let plugin = runtime(host.clone(), None);
    for (name, args) in [
        ("set_alarm", json!({"time":"1s"})),
        ("cancel_alarm", json!({"id":"missing"})),
    ] {
        assert!(call(&plugin, name, args, false).await.is_err());
    }
    assert_eq!(
        call(&plugin, "list_alarms", json!({}), false)
            .await
            .unwrap(),
        json!({"ok":true,"alarms":[]})
    );
    let denied = runtime(host.clone(), Some(Capabilities::default()));
    assert!(call(&denied, "set_alarm", json!({"time":"1s"}), true)
        .await
        .is_err());
    assert!(call(&denied, "list_alarms", json!({}), false)
        .await
        .is_err());
    let grants: Capabilities = serde_json::from_value(json!({"system":{"schedule":true}})).unwrap();
    let schedule_only = runtime(host.clone(), Some(grants));
    let created = call(&schedule_only, "set_alarm", json!({"time":"1s"}), true)
        .await
        .unwrap();
    assert!(created["ok"].as_bool().unwrap());
    let arguments = host.tasks.lock().unwrap()[0].arguments.clone();
    assert!(schedule_only
        .call_command(
            "deliver",
            &arguments,
            sai_plugin_runtime::InvocationContext {
                allow_writes: true,
                ..Default::default()
            }
        )
        .await
        .is_err());
    assert!(host.notices.lock().unwrap().is_empty());
}

/// 【闹钟契约测试】【音频校验】保存解析后的绝对路径，拒绝目录、空文件、超限和错误扩展名。
/// @returns 无
#[tokio::test]
async fn alarm_audio_uses_authorized_canonical_files() {
    let host = Arc::new(AlarmHost::default());
    let plugin = runtime(host.clone(), None);
    for name in [
        "missing.wav",
        "folder.wav",
        "empty.wav",
        "large.wav",
        "image.png",
    ] {
        assert!(
            call(
                &plugin,
                "set_alarm",
                json!({"time":"30s","audio_file":name}),
                true
            )
            .await
            .is_err(),
            "{name}"
        );
    }
    assert!(host.tasks.lock().unwrap().is_empty());
    let result = call(
        &plugin,
        "set_alarm",
        json!({"time":"30s","audio_file":"  audio.WAV  "}),
        true,
    )
    .await
    .unwrap();
    assert_eq!(result["audio_file"], audio_path("audio.WAV"));
    let grants: Capabilities =
        serde_json::from_value(json!({"system":{"schedule":true,"notify":true}})).unwrap();
    let no_read = runtime(host.clone(), Some(grants));
    assert!(call(
        &no_read,
        "set_alarm",
        json!({"time":"1s","audio_file":"audio.WAV"}),
        true
    )
    .await
    .is_err());
    assert!(call(
        &no_read,
        "set_alarm",
        json!({"time":"1s","audio_file":" "}),
        true
    )
    .await
    .is_ok());
}

/// 【闹钟契约测试】【分页与旧标识】查询所有活动任务，旧标识可取消，终态与取消中记录不再显示。
/// @returns 无
#[tokio::test]
async fn alarm_lists_pages_and_cancels_legacy_identifiers() {
    let host = Arc::new(AlarmHost::default());
    for index in 0..35 {
        let payload = json!({"time":"30s","label":format!("reminder-{index}"),"audio_file":null,"legacy_id":format!("alarm-{index}")}).to_string();
        host.tasks.lock().unwrap().push(task(
            index,
            if index == 1 {
                ScheduledStatus::Running
            } else if index == 2 {
                ScheduledStatus::Completed
            } else if index == 3 {
                ScheduledStatus::Cancelling
            } else {
                ScheduledStatus::Scheduled
            },
            &payload,
        ));
    }
    let plugin = runtime(host.clone(), None);
    let result = call(&plugin, "list_alarms", json!({}), false)
        .await
        .unwrap();
    assert_eq!(result["alarms"].as_array().unwrap().len(), 33);
    assert_eq!(result["alarms"][0]["id"], "alarm-0");
    assert_eq!(result["alarms"][1]["status"], "ringing");
    assert_eq!(
        call(&plugin, "cancel_alarm", json!({"id":" alarm-34 "}), true)
            .await
            .unwrap(),
        json!({"ok":true,"id":"alarm-34","removed":true})
    );
    assert_eq!(
        call(&plugin, "cancel_alarm", json!({"id":"alarm-34"}), true)
            .await
            .unwrap(),
        json!({"ok":false,"id":"alarm-34","removed":false})
    );
    assert_eq!(
        call(&plugin, "cancel_alarm", json!({"id":"unknown"}), true)
            .await
            .unwrap(),
        json!({"ok":false,"id":"unknown","removed":false})
    );
    assert!(call(&plugin, "cancel_alarm", json!({"id":" "}), true)
        .await
        .is_err());
}
