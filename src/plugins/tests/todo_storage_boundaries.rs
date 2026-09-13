use super::todo_support::{call, context, installed, item, record_path, snapshot};
use crate::state::StateStore;
use sai_plugin_runtime::host::{PluginHost, StorageRequest};
use serde_json::json;

/// 【待办存储测试】【结果预算】合法大条目同时出现在 changed 和 items 时仍必须成功返回
/// @returns 无；超出存储预算的后续调用不能提交任何数据
#[tokio::test]
async fn todo_storage_large_valid_item_returns_success_before_retry_is_possible() {
    let root = tempfile::tempdir().unwrap();
    let (paths, plugin, _) = installed(root.path());
    let store = StateStore::new(&paths).unwrap();
    let scope = store.state_dir().display().to_string();
    let text = "x".repeat(200000);
    let result = call(
        &plugin,
        root.path(),
        &scope,
        json!({"action":"add","text":text}),
    )
    .await;
    assert_eq!(result["changed"][0]["text"], text);
    assert_eq!(result["items"][0]["text"], text);
    let saved = std::fs::read(record_path(&paths, &scope)).unwrap();
    assert!(plugin
        .call_tool(
            "todo",
            json!({"action":"add","text":"x".repeat(100000)}),
            context(root.path(), &scope, true)
        )
        .await
        .is_err());
    assert_eq!(std::fs::read(record_path(&paths, &scope)).unwrap(), saved);
}

/// 【待办存储测试】【新记录校验】损坏和重复标识不能触发旧数据回退或覆盖原文
/// @returns 无；修复新记录之后原实例可以继续调用
#[tokio::test]
async fn todo_storage_corruption_preserves_authoritative_record() {
    let root = tempfile::tempdir().unwrap();
    let (paths, runtime, _) = installed(root.path());
    let store = StateStore::new(&paths).unwrap();
    let scope = store.state_dir().display().to_string();
    let record = record_path(&paths, &scope);
    std::fs::write(
        store.state_dir().join("todos.json"),
        json!([item("legacy", "pending")]).to_string(),
    )
    .unwrap();
    let invalid = [
        "{".to_string(),
        "[]".into(),
        "{}".into(),
        json!({"version":2,"items":[],"history":[]}).to_string(),
        json!({"version":1,"items":[item("a","pending"),item("a","pending")],"history":[]})
            .to_string(),
        json!({"version":1,"items":[],"history":[{"archived_at":"old","items":[{}]}]}).to_string(),
    ];
    for text in invalid {
        std::fs::write(&record, &text).unwrap();
        assert!(runtime
            .call_tool(
                "todo",
                json!({"action":"add","text":"new"}),
                context(root.path(), &scope, true)
            )
            .await
            .is_err());
        assert_eq!(std::fs::read_to_string(&record).unwrap(), text);
    }
    std::fs::write(&record, r#"{"version":1,"items":[],"history":[]}"#).unwrap();
    assert_eq!(
        snapshot(&runtime, root.path(), &scope).await["items"],
        json!([])
    );
}

/// 【待办存储测试】【字节预算】公共记录和显式导入的合并状态都受限，失败时不发布新状态
/// @returns 无；原始文件在超限错误之后保持原样
#[tokio::test]
async fn todo_storage_rejects_oversized_records_and_combined_legacy_data() {
    let root = tempfile::tempdir().unwrap();
    let (paths, runtime, host) = installed(root.path());
    let store = StateStore::new(&paths).unwrap();
    let scope = store.state_dir().display().to_string();
    let ctx = context(root.path(), &scope, true);
    let path = record_path(&paths, &scope);
    let bytes = vec![b' '; 262145];
    std::fs::write(&path, &bytes).unwrap();
    assert!(runtime
        .call_command("snapshot", "", ctx.clone())
        .await
        .is_err());
    assert_eq!(std::fs::read(&path).unwrap(), bytes);
    std::fs::remove_file(&path).unwrap();
    let mut long = item("long", "pending");
    long["text"] = json!("x".repeat(140000));
    let items = json!([long]).to_string();
    let history = json!([{"archived_at":"old","items":[long]}]).to_string();
    std::fs::write(store.state_dir().join("todos.json"), &items).unwrap();
    std::fs::write(store.state_dir().join("todos.history.json"), &history).unwrap();
    assert_eq!(
        snapshot(&runtime, root.path(), &scope).await["items"],
        json!([])
    );
    let value = json!({"version":0,"items":serde_json::from_str::<serde_json::Value>(&items).unwrap(),"history":serde_json::from_str::<serde_json::Value>(&history).unwrap()});
    assert!(runtime
        .call_command("import", &json!({"state":value}).to_string(), ctx.clone())
        .await
        .is_err());
    assert_eq!(
        snapshot(&runtime, root.path(), &scope).await["items"],
        json!([])
    );
    assert_eq!(
        std::fs::read_to_string(store.state_dir().join("todos.json")).unwrap(),
        items
    );
    let caps = runtime.manifest().capabilities.clone();
    assert!(host
        .storage(
            StorageRequest::CompareExchange {
                key: "plan".into(),
                expected: json!("x".repeat(262144)),
                value: json!(null),
            },
            &scope,
            &caps
        )
        .is_err());
}

/// 【待办存储测试】【锁忙恢复】正式锁被占用时立即报错，释放后重试只新增一次
/// @returns 无；失败调用不创建计划或遗留发布临时文件
#[tokio::test]
async fn todo_storage_busy_lock_preserves_state_and_allows_retry() {
    let root = tempfile::tempdir().unwrap();
    let (paths, runtime, _) = installed(root.path());
    let store = StateStore::new(&paths).unwrap();
    let scope = store.state_dir().display().to_string();
    let lock = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(paths.state_dir.join(".plugin-state.lock"))
        .unwrap();
    lock.try_lock().unwrap();
    let error = runtime
        .call_tool(
            "todo",
            json!({"action":"add","text":"once"}),
            context(root.path(), &scope, true),
        )
        .await
        .unwrap_err();
    assert!(format!("{error:#}").contains("busy"), "{error:#}");
    assert!(!record_path(&paths, &scope).exists());
    lock.unlock().unwrap();
    assert_eq!(
        call(
            &runtime,
            root.path(),
            &scope,
            json!({"action":"add","text":"once"})
        )
        .await["items"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    assert!(std::fs::read_dir(store.state_dir())
        .unwrap()
        .all(|entry| !entry
            .unwrap()
            .file_name()
            .to_string_lossy()
            .ends_with(".tmp")));
}

/// 【待办存储测试】【拒绝链接】公共记录和命名空间目录不能通过链接访问其他目录
/// @returns 无；外部文件原文保持不变
#[cfg(unix)]
#[tokio::test]
async fn todo_storage_rejects_file_and_directory_symlinks() {
    let root = tempfile::tempdir().unwrap();
    let (paths, runtime, _) = installed(root.path());
    let store = StateStore::new(&paths).unwrap();
    let scope = store.state_dir().display().to_string();
    let outside = root.path().join("outside.json");
    std::fs::write(&outside, "[]").unwrap();
    for path in [record_path(&paths, &scope)] {
        std::os::unix::fs::symlink(&outside, &path).unwrap();
        assert!(runtime
            .call_command("snapshot", "", context(root.path(), &scope, false))
            .await
            .is_err());
        assert_eq!(std::fs::read_to_string(&outside).unwrap(), "[]");
        std::fs::remove_file(path).unwrap();
    }
    let linked = record_path(&paths, "linked-session")
        .parent()
        .unwrap()
        .to_path_buf();
    std::fs::remove_dir(&linked).unwrap();
    std::os::unix::fs::symlink(record_path(&paths, &scope).parent().unwrap(), &linked).unwrap();
    assert!(runtime
        .call_command(
            "snapshot",
            "",
            context(root.path(), "linked-session", false)
        )
        .await
        .is_err());
    assert!(!record_path(&paths, &scope).exists());
}

/// 【待办存储测试】【特殊文件】目录和命名管道明确拒绝，读取管道不能阻塞查询
/// @returns 无；特殊文件仍保持原类型
#[cfg(unix)]
#[tokio::test]
async fn todo_storage_rejects_special_files_without_blocking() {
    use std::os::unix::ffi::OsStrExt;
    let root = tempfile::tempdir().unwrap();
    let (paths, runtime, _) = installed(root.path());
    let store = StateStore::new(&paths).unwrap();
    let scope = store.state_dir().display().to_string();
    for path in [record_path(&paths, &scope)] {
        std::fs::create_dir(&path).unwrap();
        assert!(runtime
            .call_command("snapshot", "", context(root.path(), &scope, false))
            .await
            .is_err());
        std::fs::remove_dir(&path).unwrap();
        let c_path = std::ffi::CString::new(path.as_os_str().as_bytes()).unwrap();
        assert_eq!(unsafe { libc::mkfifo(c_path.as_ptr(), 0o600) }, 0);
        let result = tokio::time::timeout(
            std::time::Duration::from_secs(2),
            runtime.call_command("snapshot", "", context(root.path(), &scope, false)),
        )
        .await
        .unwrap();
        assert!(result.is_err());
        std::fs::remove_file(path).unwrap();
    }
}
