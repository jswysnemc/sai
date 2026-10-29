//! 嵌入请求屏障等待：区分作业提前结束和调度超时，不改业务锁行为。

use super::knowledge_host::KnowledgeHost;
use anyhow::{bail, Context, Result};
use std::time::Duration;
use tokio::task::JoinHandle;

/// 【知识库嵌入测试】【执行器期限】限制真实死锁，不将共享执行器和文件系统耗时作为业务断言。
pub(super) const CONCURRENCY_TIMEOUT: Duration = Duration::from_secs(30);

/// 【知识库嵌入测试】【请求屏障】等待真实宿主暂停请求，作业提前结束时立即报告结果。
/// 参数: `host` 为受控宿主，`worker` 为重建作业；返回: 已到达屏障或明确的失败原因。
pub(super) async fn wait_for_http(
    host: &KnowledgeHost,
    worker: &mut JoinHandle<Result<String>>,
) -> Result<()> {
    tokio::time::timeout(CONCURRENCY_TIMEOUT, async {
        tokio::select! {
            _ = host.http_started.notified() => Ok(()),
            result = worker => bail!("embedding worker ended before the HTTP barrier: {result:?}"),
        }
    })
    .await
    .context("embedding worker did not reach the HTTP barrier before the test deadline")?
}

/// 【知识库嵌入测试】【慢启动回归】调度等待超过旧期限后，仍执行真实重建和并发修改断言。
/// 参数: 无；返回: 无，旧向量不得在修改后发布。
#[tokio::test]
async fn delayed_worker_still_reaches_http_barrier_and_rejects_stale_vectors() {
    use super::knowledge_support::*;
    use sai_plugin_runtime::host::HttpResponse;
    use serde_json::json;
    use std::sync::atomic::Ordering;

    let root = tempfile::tempdir().unwrap();
    seed(root.path(), &json!({"note.md":"old body\n"}), true);
    semantic_row(root.path(), "note.md", "old vector", "[0,1]");
    let host = KnowledgeHost::new(root.path());
    host.pause_http.store(true, Ordering::SeqCst);
    host.responses.lock().unwrap().push_back(HttpResponse {
        status: 200,
        text: r#"{"data":[{"embedding":[1,0]}]}"#.into(),
        headers: Default::default(),
    });
    let worker = configured(
        root.path(),
        &embedding_config(),
        json!({}),
        host.clone(),
        "",
        |_| {},
    );
    let editor = configured(
        root.path(),
        &embedding_config(),
        json!({}),
        host.clone(),
        "",
        |_| {},
    );
    let directory = root.path().to_path_buf();
    let mut job = tokio::spawn(async move {
        // 1. 【知识库嵌入测试】【受控调度】模拟繁忙 Windows 执行器中的作业启动延迟
        tokio::time::sleep(Duration::from_secs(4)).await;
        command(&worker, &directory, "embed-reindex", json!({})).await
    });
    if let Err(error) = wait_for_http(&host, &mut job).await {
        job.abort();
        panic!("delayed embedding worker should reach the HTTP barrier: {error:#}");
    }
    // 2. 【知识库嵌入测试】【真实竞争】网络仍暂停时修改文件，验证数据锁没有跨越网络等待
    tokio::time::timeout(
        CONCURRENCY_TIMEOUT,
        call(
            &editor,
            root.path(),
            "edit_knowledge_base_file",
            json!({"file_name":"note.md","start_line":1,"end_line":1,"replacement":"new body"}),
            true,
        ),
    )
    .await
    .unwrap()
    .unwrap();
    assert!(semantic_rows(root.path()).is_empty());
    host.http_release.notify_one();
    assert_eq!(job.await.unwrap().unwrap(), "indexed semantic chunks: 0");
    assert!(semantic_rows(root.path()).is_empty());
}

/// 【知识库嵌入测试】【提前退出回归】保留真实作业错误，不能将提前失败伪装为请求超时。
/// 参数: 无；返回: 无。
#[tokio::test]
async fn worker_failure_before_http_is_reported_immediately() {
    let root = tempfile::tempdir().unwrap();
    let host = KnowledgeHost::new(root.path());
    let mut worker = tokio::spawn(async { Err(anyhow::anyhow!("fixture worker failure")) });
    let error = wait_for_http(&host, &mut worker).await.unwrap_err();
    assert!(
        error.to_string().contains("fixture worker failure"),
        "{error:#}"
    );
}

/// 【知识库嵌入测试】【通知回归】先到的请求通知保留许可，不依赖等待任务先获得调度。
/// 参数: 无；返回: 无。
#[tokio::test]
async fn http_notification_before_wait_is_preserved() {
    let root = tempfile::tempdir().unwrap();
    let host = KnowledgeHost::new(root.path());
    host.http_started.notify_one();
    let mut worker = tokio::spawn(std::future::pending::<Result<String>>());
    let result = wait_for_http(&host, &mut worker).await;
    worker.abort();
    assert!(result.is_ok(), "{result:?}");
}
