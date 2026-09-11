#[path = "file_removal/support.rs"]
mod support;

use serde_json::json;
use std::{
    sync::{atomic::Ordering, Arc},
    time::Duration,
};
use support::*;

/// 【删除生命周期测试】【总时限】回调超时必须释放宿主 Future，后续调用可以恢复
/// @returns 无；两种删除均受同一回调总时限约束
#[tokio::test]
async fn removal_deadlines_drop_the_host_future_and_allow_recovery() {
    for operation in ["remove_file", "trash_file"] {
        let host = Arc::new(Host::default());
        host.blocking.store(true, Ordering::SeqCst);
        let plugin = runtime(SOURCE, host.clone(), |limits| limits.timeout_ms = 100);
        let error = tokio::time::timeout(
            Duration::from_secs(2),
            plugin.call_tool("run", json!({"operation":operation}), context(true)),
        )
        .await
        .unwrap()
        .unwrap_err();
        assert!(format!("{error:#}").contains("timed out"), "{error:#}");
        tokio::time::timeout(Duration::from_secs(2), host.released.notified())
            .await
            .unwrap();
        assert_eq!(host.active.load(Ordering::SeqCst), 0);
        host.blocking.store(false, Ordering::SeqCst);
        assert_eq!(
            plugin
                .call_tool("run", json!({"operation":operation}), context(true))
                .await
                .unwrap(),
            "false"
        );
    }
}

/// 【删除生命周期测试】【外部取消】取消调用后不能把旧宿主操作带入下一回调
/// @returns 无；可观察到 Future 释放及实例恢复
#[tokio::test]
async fn cancelled_removals_release_the_inflight_operation() {
    let host = Arc::new(Host::default());
    host.blocking.store(true, Ordering::SeqCst);
    let plugin = Arc::new(runtime(SOURCE, host.clone(), |_| {}));
    let instance = plugin.clone();
    let call =
        tokio::spawn(async move { instance.call_tool("run", json!({}), context(true)).await });
    tokio::time::timeout(Duration::from_secs(2), host.entered.notified())
        .await
        .unwrap();
    call.abort();
    assert!(call.await.unwrap_err().is_cancelled());
    tokio::time::timeout(Duration::from_secs(2), host.released.notified())
        .await
        .unwrap();
    assert_eq!(host.active.load(Ordering::SeqCst), 0);
    host.blocking.store(false, Ordering::SeqCst);
    assert_eq!(
        plugin
            .call_tool("run", json!({}), context(true))
            .await
            .unwrap(),
        "false"
    );
}

/// 【删除生命周期测试】【初始化边界】模块初始化不能执行文件删除
/// @returns 无；加载失败且没有进入宿主
#[test]
fn initialization_cannot_remove_or_trash_files() {
    for operation in ["remove_file", "trash_file"] {
        let host = Arc::new(Host::default());
        assert!(load(
            &format!("sai.fs.{operation}('output/a')\n{SOURCE}"),
            host.clone(),
            capabilities(),
            capabilities(),
            |_| {}
        )
        .is_err());
        assert!(host.calls.lock().unwrap().is_empty());
    }
}
