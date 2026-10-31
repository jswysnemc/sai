use super::super::runtime::RuntimeGuard;
use super::*;
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc, Mutex,
};

/// 【微信网关】【测试客户端】无参数，返回不连接外网的固定账号客户端。
fn client() -> WeixinBotClient {
    WeixinBotClient::new(
        "http://127.0.0.1:1".into(),
        "http://127.0.0.1:1".into(),
        "fixture-token".into(),
        None,
        false,
    )
}

/// 【微信网关】【重复启动回归】无参数，返回无；已有租约不启动，释放后状态立即停止。
#[tokio::test]
async fn running_gateway_is_reused_and_released_lease_is_stopped() {
    let root = tempfile::tempdir().unwrap();
    let paths = SaiPaths::for_tests(root.path());
    let client = client();
    let guard = RuntimeGuard::acquire(&paths, &client).unwrap();
    ensure_with(&paths, &client, || async { panic!("must not start twice") })
        .await
        .unwrap();
    assert!(RuntimeGuard::acquire(&paths, &client).is_err());
    drop(guard);
    assert!(!runtime::is_running(&paths, &client).unwrap());
}

/// 【微信网关】【并发启动回归】无参数，返回无；并发请求仅执行一次启动操作。
#[tokio::test]
async fn concurrent_requests_start_only_one_gateway() {
    let root = tempfile::tempdir().unwrap();
    let paths = SaiPaths::for_tests(root.path());
    let guard = Arc::new(Mutex::new(None));
    let count = Arc::new(AtomicUsize::new(0));
    let mut tasks = tokio::task::JoinSet::new();
    for _ in 0..8 {
        let paths = paths.clone();
        let guard = guard.clone();
        let count = count.clone();
        tasks.spawn(async move {
            let client = client();
            ensure_with(&paths, &client, || async {
                count.fetch_add(1, Ordering::SeqCst);
                tokio::time::sleep(Duration::from_millis(20)).await;
                *guard.lock().unwrap() = Some(RuntimeGuard::acquire(&paths, &client)?);
                Ok(())
            })
            .await
        });
    }
    while let Some(result) = tasks.join_next().await {
        result.unwrap().unwrap();
    }
    assert_eq!(count.load(Ordering::SeqCst), 1);
}

/// 【微信网关】【启动失败回归】无参数，返回无；启动错误可见且不留下虚假运行状态。
#[tokio::test]
async fn startup_failure_is_reported_and_does_not_mark_running() {
    let root = tempfile::tempdir().unwrap();
    let paths = SaiPaths::for_tests(root.path());
    let client = client();
    let error = ensure_with(&paths, &client, || async {
        anyhow::bail!("fixture launch failure")
    })
    .await
    .unwrap_err();
    assert!(format!("{error:#}").contains("fixture launch failure"));
    assert!(!runtime::is_running(&paths, &client).unwrap());
}

/// 【微信网关】【账号隔离回归】无参数，返回无；运行账号不一致时报错，不把旧服务当成当前账号。
#[tokio::test]
async fn a_different_account_is_not_reused() {
    let root = tempfile::tempdir().unwrap();
    let paths = SaiPaths::for_tests(root.path());
    let _guard = RuntimeGuard::acquire(&paths, &client()).unwrap();
    let other = WeixinBotClient::new(
        "http://127.0.0.1:1".into(),
        "".into(),
        "other-token".into(),
        None,
        false,
    );
    let error = ensure_with(&paths, &other, || async {
        panic!("must not launch another account")
    })
    .await
    .unwrap_err();
    assert!(error.to_string().contains("different credentials"));
    assert!(!error.to_string().contains("other-token"));
}

/// 【微信网关】【崩溃残留回归】无参数，返回无；仅有旧状态文件不能阻止重新启动。
#[test]
fn stale_identity_without_a_lock_is_not_running() {
    let root = tempfile::tempdir().unwrap();
    let paths = SaiPaths::for_tests(root.path());
    let client = client();
    let guard = RuntimeGuard::acquire(&paths, &client).unwrap();
    let identity = paths.state_dir.join("gateways/weixin-runtime.json");
    let bytes = std::fs::read(&identity).unwrap();
    drop(guard);
    std::fs::write(identity, bytes).unwrap();
    assert!(!runtime::is_running(&paths, &client).unwrap());
}
