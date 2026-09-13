use crate::native::Budget;
use mlua::Lua;
use std::{sync::Arc, time::Duration};
use tokio_util::sync::CancellationToken;

/// 【插件计算】【原生调度】异步等待结束时撤销计算，实际线程持有输入和预留直到退出
/// @param lua 虚拟机；timeout_ms 为局部时限；label 为错误类别；operation 为拥有全部租约的工作
/// @returns 当前调用仍有效时交付完整结果，排队时间包含在期限内
pub(super) async fn work<T: Send + 'static>(
    lua: &Lua,
    timeout_ms: u64,
    label: &'static str,
    operation: impl FnOnce(Budget) -> anyhow::Result<T> + Send + 'static,
) -> mlua::Result<T> {
    // 1. 【插件计算】【取消绑定】捕获可信预算，Future 被释放时通知排队中和运行中的工作
    let execution = super::budget::worker(lua)?;
    let cancel = CancellationToken::new();
    let _cancel_on_drop = cancel.clone().drop_guard();
    let budget: Budget = Arc::new(move |units| {
        anyhow::ensure!(!cancel.is_cancelled(), "{label} operation cancelled");
        execution(units)
    });
    let running = budget.clone();
    // 2. 【插件计算】【线程所有权】闭包拥有输入和工作租约，取消排队任务不执行实际转换
    let worker = tokio::task::spawn_blocking(move || {
        running(0)?;
        let result = operation(running.clone());
        running(0)?;
        result
    });
    // 3. 【插件计算】【结果交付】局部超时不提前归还线程资源，迟到结果不能交付给 Lua
    let result = tokio::time::timeout(Duration::from_millis(timeout_ms), worker)
        .await
        .map_err(|_| mlua::Error::runtime(format!("{label} operation timed out")))?
        .map_err(super::system::error)?;
    budget(0).map_err(super::system::error)?;
    result.map_err(super::system::error)
}
