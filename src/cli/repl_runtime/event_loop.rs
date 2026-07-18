use super::ReplRuntime;
use anyhow::Result;

/// 在流式事件循环 tick 中采样尺寸并执行到期 reflow 与 live 刷新。
///
/// 参数:
/// - `runtime`: 当前 REPL 运行期
///
/// 返回:
/// - 处理是否成功
pub(crate) fn process_stream_tick(runtime: &mut ReplRuntime) -> Result<()> {
    runtime.observe_terminal_size(true)?;
    runtime.maybe_reflow_due(true)?;
    runtime.tick_live()?;
    runtime.tick_subagents().map(|_| ())
}
