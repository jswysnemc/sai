pub(crate) mod atomic_write;
pub(crate) mod line_endings;
pub(crate) mod locks;

use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

tokio::task_local! {
    static CANCELLED: Arc<AtomicBool>;
}

struct Cancellation(Arc<AtomicBool>);

impl Drop for Cancellation {
    /// 【文件编辑】【取消标记】异步调用取消后阻止等待锁的旧编辑工具继续写入。
    /// 参数: 无；返回: 无。
    fn drop(&mut self) {
        self.0.store(true, Ordering::Release);
    }
}

/// 【文件编辑】【取消检查】同步工具在锁等待及原子替换前检查所属调用是否仍然有效。
/// 参数: 无；返回: 未取消时成功，直接同步调用不附加取消限制。
pub(crate) fn ensure_active() -> anyhow::Result<()> {
    anyhow::ensure!(
        !CANCELLED
            .try_with(|flag| flag.load(Ordering::Acquire))
            .unwrap_or(false),
        "file edit cancelled before commit"
    );
    Ok(())
}

/// 【文件编辑】【线程分流】在后台线程执行有锁读改写，保留会话工作目录且不阻塞其他工具。
/// 参数: args 为工具参数，edit 为同步编辑函数；返回: 编辑结果。
pub(crate) async fn blocking_edit(
    args: serde_json::Value,
    edit: fn(serde_json::Value) -> anyhow::Result<String>,
) -> anyhow::Result<String> {
    let directory = crate::runtime_cwd::current_dir()?;
    let cancellation = Cancellation(Arc::new(AtomicBool::new(false)));
    let flag = cancellation.0.clone();
    tokio::task::spawn_blocking(move || {
        crate::runtime_cwd::with_directory(directory, || CANCELLED.sync_scope(flag, || edit(args)))
    })
    .await?
}
