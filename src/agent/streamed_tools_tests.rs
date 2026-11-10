use super::*;
use crate::{llm::ToolCallFunction, tools::ToolSpec};
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};

/// 【工具流】【调用夹具】构造完整参数调用；参数为 ID 与名称，返回调用。
fn call(id: &str, name: &str) -> ToolCall {
    ToolCall {
        id: id.into(),
        kind: "function".into(),
        function: ToolCallFunction {
            name: name.into(),
            arguments: "{}".into(),
        },
    }
}

/// 【工具流】【提前执行回归】响应未完成前读取已开始，正式执行复用结果且写调用阻止后续预读。
/// 参数：无；返回断言结果。
#[tokio::test]
async fn reads_start_before_response_completion_and_are_consumed_once() {
    let count = Arc::new(AtomicUsize::new(0));
    let observed = count.clone();
    let mut tools = ToolRegistry::new();
    tools.register(ToolSpec::new(
        "read_file",
        "read",
        serde_json::json!({"type":"object"}),
        move |_| {
            let observed = observed.clone();
            async move {
                observed.fetch_add(1, Ordering::SeqCst);
                Ok("found".into())
            }
        },
    ));
    let visibility = ToolVisibility::new(Vec::new());
    let cwd = std::env::current_dir().unwrap();
    let read = call("first", "read_file");
    let mut cache = StreamedTools::default();
    cache.ready(0, read.clone(), &tools, &visibility, &cwd);
    tokio::task::yield_now().await;
    assert_eq!(count.load(Ordering::SeqCst), 1);
    cache.ready(1, call("write", "write_file"), &tools, &visibility, &cwd);
    cache.ready(2, call("later", "read_file"), &tools, &visibility, &cwd);
    assert!(cache.take(&call("later", "read_file")).await.is_none());
    cache.retain_confirmed(std::slice::from_ref(&read));
    assert_eq!(cache.take(&read).await.unwrap().output, "found");
    assert!(cache.take(&read).await.is_none());
    assert_eq!(count.load(Ordering::SeqCst), 1);
}

/// 【工具流】【重试回归】响应撤销或参数变化必须取消未完成读取，不能跨响应复用。
/// 参数：无；返回断言结果。
#[tokio::test]
async fn discarded_response_aborts_unfinished_reads() {
    let mut tools = ToolRegistry::new();
    tools.register(ToolSpec::new(
        "read_file",
        "read",
        serde_json::json!({"type":"object"}),
        |_| async { std::future::pending::<Result<String>>().await },
    ));
    let mut cache = StreamedTools::default();
    cache.ready(
        0,
        call("read", "read_file"),
        &tools,
        &ToolVisibility::new(Vec::new()),
        &std::env::current_dir().unwrap(),
    );
    let abort = cache.pending.lock().unwrap()["read"].handle.abort_handle();
    cache.retain_confirmed(&[call("write", "write_file"), call("read", "read_file")]);
    tokio::task::yield_now().await;
    assert!(abort.is_finished());
}
