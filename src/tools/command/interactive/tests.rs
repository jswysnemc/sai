use super::*;

/// 【交互命令】【PTY 回归】真实 Shell 收到终端输入，跨会话写入被拒绝，结束后清除会话。
#[cfg(unix)]
#[tokio::test]
async fn interactive_input_is_scoped_and_completed_sessions_are_removed() {
    let root = tempfile::tempdir().unwrap();
    let owner = BackgroundRuntimeOwner::session(&format!("pty-test-{}", uuid::Uuid::new_v4()));
    let args = json!({"command":"test -t 0 || exit 9; printf 'ready\\n'; IFS= read -r value; printf 'received:%s\\n' \"$value\"", "cwd":root.path(), "yield_time_ms":100});
    let initial: Value =
        serde_json::from_str(&start(&args, "sh", Some(&owner)).await.unwrap()).unwrap();
    assert_eq!(initial["running"], true);
    let id = initial["session_id"].as_str().unwrap();
    assert!(interact(
        "session:unrelated",
        &json!({"session_id":id,"chars":"wrong\n"})
    )
    .await
    .is_err());
    assert!(sessions().lock().unwrap().contains_key(id));
    let final_result: Value = serde_json::from_str(
        &interact(
            &owner_key(&owner),
            &json!({"session_id":id,"chars":"中文输入\n","yield_time_ms":1000}),
        )
        .await
        .unwrap(),
    )
    .unwrap();
    assert_eq!(final_result["exit_code"], 0);
    assert_eq!(final_result["running"], false);
    assert!(final_result["stdout"]
        .as_str()
        .unwrap()
        .contains("received:中文输入"));
    assert!(final_result["session_id"].is_null());
    assert!(!sessions().lock().unwrap().contains_key(id));
}

/// 【交互命令】【取消回归】启动工具取消后清理子进程和注册记录，不遗留后台 Shell。
#[cfg(unix)]
#[tokio::test]
async fn cancelled_interactive_start_kills_the_process_tree() {
    let root = tempfile::tempdir().unwrap();
    let owner = BackgroundRuntimeOwner::session(&format!("pty-cancel-{}", uuid::Uuid::new_v4()));
    let key = owner_key(&owner);
    let args = json!({"command":"sleep 60 & wait", "cwd":root.path(), "yield_time_ms":60000});
    let worker = tokio::spawn(async move { start(&args, "sh", Some(&owner)).await });
    let pid = tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            if let Some(pid) = sessions()
                .lock()
                .unwrap()
                .values()
                .find(|(session, _)| session.owner == key)
                .map(|(session, _)| session.pid)
            {
                break pid;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    worker.abort();
    let _ = worker.await;
    tokio::time::timeout(Duration::from_secs(3), async {
        while crate::tools::command::process_exists(pid) {
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .unwrap();
    assert!(!sessions()
        .lock()
        .unwrap()
        .values()
        .any(|(session, _)| session.owner == key));
}

/// 【交互命令】【终止回归】显式终止可以停止等待输入的命令，并返回最终状态。
#[cfg(unix)]
#[tokio::test]
async fn explicit_termination_releases_interactive_session() {
    let owner = BackgroundRuntimeOwner::session(&format!("pty-stop-{}", uuid::Uuid::new_v4()));
    let initial: Value = serde_json::from_str(
        &start(
            &json!({"command":"read value", "yield_time_ms":0}),
            "sh",
            Some(&owner),
        )
        .await
        .unwrap(),
    )
    .unwrap();
    let id = initial["session_id"].as_str().unwrap();
    let result: Value = serde_json::from_str(
        &interact(
            &owner_key(&owner),
            &json!({"session_id":id,"terminate":true,"yield_time_ms":1000}),
        )
        .await
        .unwrap(),
    )
    .unwrap();
    assert_eq!(result["running"], false);
    assert!(result["exit_code"].as_u64().unwrap() > 0);
    assert!(!sessions().lock().unwrap().contains_key(id));
}

/// 【交互命令】【沙箱回归】PTY 保留只读沙箱，终端输入能力不能写入工作区文件。
#[cfg(target_os = "linux")]
#[test]
fn pty_keeps_readonly_sandbox_when_backend_is_available() {
    if !crate::sandbox::backend_availability().available {
        return;
    }
    let root = tempfile::tempdir().unwrap();
    let policy = crate::sandbox::SandboxPolicy::resolve(
        &crate::sandbox::SandboxSettings::default(),
        crate::sandbox::FileAccess::ReadOnly,
        root.path(),
        None,
    );
    let session = InteractiveSession::spawn(
        "sandbox-test".into(),
        "printf forbidden > should-not-exist",
        "sh",
        root.path(),
        Some(&policy),
    )
    .unwrap();
    let deadline = Instant::now() + Duration::from_secs(3);
    while session.status().unwrap().0.is_none() && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(session.status().unwrap().0.is_some_and(|code| code != 0));
    assert!(!root.path().join("should-not-exist").exists());
}
