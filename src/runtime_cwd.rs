use std::future::Future;
use std::path::PathBuf;

tokio::task_local! {
    static RUNTIME_CWD: PathBuf;
}

/// 在指定工作目录作用域内执行异步任务。
///
/// 参数:
/// - `path`: 本次任务固定使用的工作目录
/// - `future`: 需要执行的异步任务
///
/// 返回:
/// - 异步任务返回值
pub(crate) async fn scope<F>(path: PathBuf, future: F) -> F::Output
where
    F: Future,
{
    RUNTIME_CWD.scope(path, future).await
}

/// 返回当前异步运行绑定的工作目录，无绑定时回退到进程目录。
///
/// 返回:
/// - 当前工作目录
pub(crate) fn current_dir() -> std::io::Result<PathBuf> {
    RUNTIME_CWD
        .try_with(Clone::clone)
        .or_else(|_| std::env::current_dir())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 验证并行任务各自读取固定工作目录。
    #[tokio::test]
    async fn isolates_parallel_runtime_directories() {
        let first = scope(PathBuf::from("/tmp/workspace-a"), async {
            current_dir().unwrap()
        });
        let second = scope(PathBuf::from("/tmp/workspace-b"), async {
            current_dir().unwrap()
        });
        let (first, second) = tokio::join!(first, second);

        assert_eq!(first, PathBuf::from("/tmp/workspace-a"));
        assert_eq!(second, PathBuf::from("/tmp/workspace-b"));
    }
}
