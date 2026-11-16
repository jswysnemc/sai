//! 日志视口：分页模式展示通道徽章、跟随状态与行号分栏，列表模式复用同一日志行格式显示尾部输出。

use super::{
    model::{safe_text, Stream, TaskManager},
    theme::{fit, pill, stream_name, RESET},
    view::thumb,
};
use crate::i18n::text as t;
use crate::render::panel_palette::{fg, PanelPalette};
use std::ops::Range;

/// 【后台任务】【日志视口】渲染当前日志页，行号表示缓冲区序号，并在溢出时给出滑块行范围。
///
/// 参数:
/// - `manager`: 任务管理状态
/// - `width`: 正文内容宽度
/// - `height`: 可见日志行数
///
/// 返回:
/// - `(分页视觉行, 滑块相对正文的行范围)`
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

    // 1. 【后台任务】【日志顶栏】突出当前通道胶囊、缓冲区行范围与截断说明
    let stream_color = match manager.stream {
        Stream::Stdout => palette.accent,
        Stream::Stderr => palette.removed,
        Stream::All => palette.text,
    };
    let stream_pill = pill(
        &format!(" {} ", stream_name(manager.stream)),
        stream_color,
        Some(palette.focus_bg),
        true,
    );
    let follow_pill = if manager.follow {
        pill(
            &format!(" ● {} ", t("Follow", "跟随")),
            palette.added,
            Some(palette.hover_bg),
            false,
        )
    } else {
        pill(
            &format!(" ○ {} ", t("Paused", "暂停")),
            palette.muted,
            Some(palette.hover_bg),
            false,
        )
    };
    let hint = status(manager).map_or_else(
        || {
            format!(
                "{stream_pill} {follow_pill} {} {} {}-{}/{} · {}{}",
                fg(palette.muted),
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
        |(text, _)| format!("{stream_pill} {}{text}{RESET}", fg(palette.muted)),
    );
    let mut result = vec![format!(" {}", fit(&hint, width.saturating_sub(1)))];
    for (index, line) in logs.iter().enumerate().skip(offset).take(height) {
        result.push(log_row(index, line, width, &palette));
    }
    if logs.is_empty() {
        result.push(empty(&palette));
    }
    result.resize(height + 1, String::new());
    // 2. 【后台任务】【滑块定位】顶栏占用第 0 行，滑块按日志实际行数整体下移一行
    let bar = thumb(offset, height, logs.len());
    (result, bar.start + 1..bar.end + 1)
}

/// 【后台任务】【尾部输出】在列表模式下渲染选中任务最后若干行日志。
///
/// 参数:
/// - `manager`: 任务管理状态
/// - `logs`: 已解析的当前通道日志行
/// - `width`: 输出区可用列宽
/// - `height`: 可用行数
/// - `palette`: 当前面板色板
///
/// 返回:
/// - 至多 `height` 行日志文本
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

/// 【后台任务】【读取状态】输出尚未到达或读取失败时返回提示文字及错误标记。
///
/// 参数:
/// - `manager`: 任务管理状态
///
/// 返回:
/// - 需要显示占位状态时返回 `(文字, 是否错误)`
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

/// 【后台任务】【行号分栏】弱化显示五位行号与细竖线，日志正文保持高对比前景。
///
/// 参数:
/// - `index`: 缓冲区行下标
/// - `line`: 已去控制的单行日志
/// - `width`: 行宽预算
/// - `palette`: 当前面板色板
///
/// 返回:
/// - 格式化后的日志行
fn log_row(index: usize, line: &str, width: usize, palette: &PanelPalette) -> String {
    format!(
        "{}{:>5}\x1b[39m {}│\x1b[39m {}",
        fg(palette.muted),
        index + 1,
        fg(palette.border),
        fit(line, width.saturating_sub(8))
    )
}

/// 【后台任务】【空输出】当前通道暂无日志时的柔和提示。
///
/// 参数:
/// - `palette`: 当前面板色板
///
/// 返回:
/// - 单行空状态文字
fn empty(palette: &PanelPalette) -> String {
    format!(
        " {}{}\x1b[0m",
        fg(palette.muted),
        t("No output in this stream", "当前通道暂无输出")
    )
}
