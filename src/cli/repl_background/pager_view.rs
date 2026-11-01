//! 日志滚动视口，明确显示缓冲区范围、输出通道和截断状态。

use super::{
    model::{safe_text, Stream, TaskManager},
    view::fit,
};
use crate::{
    config_tui::theme::{DIM, MUTED, RESET},
    i18n::text as t,
};

/// 【后台任务】【日志视口】渲染当前日志页，行号仅表示当前缓冲区序号。
/// 参数: manager 为状态，width 为内容宽度，height 为日志行数；返回: 页内文本行。
pub(super) fn rows(manager: &mut TaskManager, width: usize, height: usize) -> Vec<String> {
    let logs = manager.log_lines();
    let offset = manager.pager_offset(height);
    manager.offset = offset;
    let truncated = manager.output.stdout_truncated || manager.output.stderr_truncated;
    let hint = if manager.output_id != manager.selected_id {
        t("Loading output...", "正在读取输出…").into()
    } else if let Some(error) = &manager.output_error {
        format!("{}: {}", t("Read failed", "读取失败"), safe_text(error))
    } else {
        format!(
            "{} {}-{}/{} · {}{}",
            t("Buffer", "缓冲区"),
            if logs.is_empty() { 0 } else { offset + 1 },
            (offset + height).min(logs.len()),
            logs.len(),
            if truncated {
                t(
                    "Tail only, earlier output omitted",
                    "仅尾部，更早输出已截断",
                )
            } else {
                t("Current output", "当前输出")
            },
            if manager.stream == Stream::All {
                t(" · grouped by stream", " · 按通道分组")
            } else {
                ""
            }
        )
    };
    let mut result = vec![format!("{MUTED}{}{RESET}", fit(&hint, width))];
    for (index, line) in logs.iter().enumerate().skip(offset).take(height) {
        result.push(format!(
            "{DIM}{:>4} │{RESET} {}",
            index + 1,
            fit(line, width.saturating_sub(7))
        ));
    }
    if logs.is_empty() {
        result.push(format!(
            "{MUTED}{}{RESET}",
            t("No output in this stream", "当前通道暂无输出")
        ));
    }
    while result.len() < height + 1 {
        result.push(String::new());
    }
    result
}
