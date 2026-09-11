use super::file_removal_support::*;
use crate::{
    paths::SaiPaths,
    plugins::{file_ops, private::PrivatePluginHost},
};
use sai_plugin_runtime::{
    host::{FileRemovalKind, FileRemovalRequest, SystemContext},
    Capabilities,
};
use serde_json::json;
use std::{future::Future, sync::Arc, time::Duration};

/// 【删除取消测试】【共用锁等待】两种删除都等待现有二进制写入锁，超时后源文件和回收站不变
/// @returns 无；释放锁后同一运行时可以继续删除
#[tokio::test]
async fn file_removal_waits_for_the_binary_write_lock_and_recovers_after_timeout() {
    let root = tempfile::tempdir().unwrap();
    let paths = SaiPaths::for_tests(root.path());
    std::fs::create_dir_all(&paths.state_dir).unwrap();
    std::fs::create_dir(root.path().join("output")).unwrap();
    std::fs::write(root.path().join("output/a"), b"keep").unwrap();
    let lock = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(paths.state_dir.join("plugin-binary-write.lock"))
        .unwrap();
    lock.lock().unwrap();
    let plugin = with_host(
        Arc::new(PrivatePluginHost::new(&paths, "file-removal")),
        json!({"remove_paths":["output"],"trash_paths":["output"]}),
        |manifest| manifest.limits.timeout_ms = 100,
    );
    for operation in ["remove_file", "trash_file"] {
        let error = tokio::time::timeout(
            Duration::from_secs(2),
            plugin.call_tool(
                "run",
                json!({"path":"output/a","operation":operation}),
                context(root.path()),
            ),
        )
        .await
        .unwrap()
        .unwrap_err();
        assert!(format!("{error:#}").contains("timed out"), "{error:#}");
        assert_eq!(
            std::fs::read(root.path().join("output/a")).unwrap(),
            b"keep"
        );
    }
    lock.unlock().unwrap();
    assert_eq!(
        plugin
            .call_tool("run", json!({"path":"output/a"}), context(root.path()))
            .await
            .unwrap(),
        "true"
    );
}

/// 【删除取消测试】【排队取消】阻塞线程尚未运行时释放调用，迟到工作不能创建锁或删除文件
/// @returns 无；唯一阻塞线程由测试门闩控制，取消后的两个任务均完成检查
#[test]
fn file_removal_cancelled_queued_workers_do_not_touch_the_filesystem() {
    let executor = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(1)
        .max_blocking_threads(1)
        .enable_all()
        .build()
        .unwrap();
    executor.block_on(async {
        let root = tempfile::tempdir().unwrap();
        let paths = SaiPaths::for_tests(root.path());
        std::fs::create_dir(root.path().join("output")).unwrap();
        std::fs::write(root.path().join("output/a"), b"keep").unwrap();
        let (entered, started) = tokio::sync::oneshot::channel();
        let (release, receiver) = std::sync::mpsc::channel();
        let blocker = tokio::task::spawn_blocking(move || {
            entered.send(()).unwrap();
            receiver.recv().unwrap();
        });
        started.await.unwrap();
        let capabilities: Capabilities = serde_json::from_value(
            json!({"system":{"remove_paths":["output"],"trash_paths":["output"]}}),
        )
        .unwrap();
        for kind in [FileRemovalKind::Permanent, FileRemovalKind::Trash] {
            let mut operation = Box::pin(file_ops::execute(
                paths.state_dir.clone(),
                FileRemovalRequest {
                    path: "output/a".into(),
                    kind,
                },
                SystemContext {
                    workdir: root.path().to_string_lossy().into_owned(),
                    allow_writes: true,
                },
                capabilities.clone(),
            ));
            std::future::poll_fn(|context| {
                assert!(operation.as_mut().poll(context).is_pending());
                std::task::Poll::Ready(())
            })
            .await;
            drop(operation);
        }
        release.send(()).unwrap();
        blocker.await.unwrap();
        tokio::task::spawn_blocking(|| {}).await.unwrap();
        assert_eq!(
            std::fs::read(root.path().join("output/a")).unwrap(),
            b"keep"
        );
        assert!(!paths.state_dir.exists());
    });
}
