use crate::render::work_status::WorkStatus;
use std::time::Duration;

/// 渲染 REPL 当前工作状态。
///
/// 参数:
/// - `status`: 当前单轮工作状态
/// - `frame`: 动画帧序号
/// - `elapsed`: 本轮自首次回应起的已持续时长
///
/// 返回:
/// - 带 ANSI 样式的状态行
pub(super) fn render(status: WorkStatus, frame: usize, elapsed: Duration) -> String {
    status.render_line(frame, elapsed)
}
