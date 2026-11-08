//! 日志视口：分页模式显示缓冲区范围与滚动位置，列表模式复用同一行格式显示尾部输出。

use super::{
    model::{safe_text, Stream, TaskManager},
    view::{fit, thumb},
};
use crate::i18n::text as t;
use crate::render::panel_palette::{fg, PanelPalette};
use std::ops::Range;

/// 【后台任务】【日志视口】渲染当前日志页，行号仅表示当前缓冲区序号；日志溢出时给出滚动滑块位置。
/// 参数: manager 为状态，width 为内容宽度，height 为日志行数；返回: 页内文本行与滑块所在的正文行范围。
pub(super) fn rows(
    manager: &mut TaskManager,
    width: usize,
    height: usize,
) -> (Vec<String>, Range<usize>) {
    let palette = PanelPalette::current();
    let logs = manager.log_lines();
    let offset = manager.pager_offset(height);
    manager.offset = offset;
    let truncated = manager.output.stdout_truncated || manager.output.stderr_truncated;
    let hint = status(manager).map_or_else(
        || {
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
        },
        |(text, _)| text,
    );
    let mut result = vec![format!(
        " {}{}\x1b[0m",
        fg(palette.muted),
        fit(&hint, width.saturating_sub(1))
    )];
    for (index, line) in logs.iter().enumerate().skip(offset).take(height) {
        result.push(log_row(index, line, width, &palette));
    }
    if logs.is_empty() {
        result.push(empty(&palette));
    }
    result.resize(height + 1, String::new());
    // 1. 【后台任务】【滚动位置】滑块按日志行计算，首行为缓冲区提示，因此整体下移一行
    let bar = thumb(offset, height, logs.len());
    (result, bar.start + 1..bar.end + 1)
}

/// 【后台任务】【尾部输出】列表模式下显示选中任务最后若干行，读取中或失败时显示单行状态。
/// 参数: manager 为状态，logs 为当前通道日志行，width 为内容宽度，height 为可用行数，palette 为主题；返回: 不超过 height 行的输出。
pub(super) fn tail_rows(
    manager: &TaskManager,
    logs: &[String],
    width: usize,
    height: usize,
    palette: &PanelPalette,
) -> Vec<String> {
    if let Some((text, error)) = status(manager) {
        let color = if error {
            palette.removed
        } else {
            palette.muted
        };
        return vec![format!(
            " {}{}\x1b[0m",
            fg(color),
            fit(&text, width.saturating_sub(1))
        )];
    }
    if logs.is_empty() {
        return vec![empty(palette)];
    }
    let start = logs.len().saturating_sub(height);
    logs.iter()
        .enumerate()
        .skip(start)
        .map(|(index, line)| log_row(index, line, width, palette))
        .collect()
}

/// 【后台任务】【读取状态】输出尚未读到或读取失败时返回提示文字及是否为错误。
/// 参数: manager 为状态；返回: 提示与错误标识，输出可用时为空。
fn status(manager: &TaskManager) -> Option<(String, bool)> {
    if manager.output_id != manager.selected_id {
        Some((t("Loading output...", "正在读取输出…").into(), false))
    } else {
        manager.output_error.as_ref().map(|error| {
            (
                format!("{}: {}", t("Read failed", "读取失败"), safe_text(error)),
                true,
            )
        })
    }
}

/// 【后台任务】【行号分栏】行号减淡、分隔线使用细线色，日志正文保持默认前景。
/// 参数: index 为缓冲区下标，line 为日志行，width 为内容宽度，palette 为主题；返回: 单行日志。
fn log_row(index: usize, line: &str, width: usize, palette: &PanelPalette) -> String {
    format!(
        "{}{:>5}\x1b[39m {}│\x1b[39m {}",
        fg(palette.muted),
        index + 1,
        fg(palette.border),
        fit(line, width.saturating_sub(8))
    )
}

/// 【后台任务】【空输出】当前通道没有日志时的减淡提示。
/// 参数: palette 为主题；返回: 提示行。
fn empty(palette: &PanelPalette) -> String {
    format!(
        " {}{}\x1b[0m",
        fg(palette.muted),
        t("No output in this stream", "当前通道暂无输出")
    )
}
