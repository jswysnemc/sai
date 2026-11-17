//! 后台任务列表视图组件：筛选胶囊、通道切换入口与单行任务条目。

use super::{
    command_style::highlight_inline,
    model::{safe_text, Filter, Stream, Task, TaskManager},
    task_style::status_style,
    theme::{fit, pill, status_glyph, stream_name, RESET},
    view::duration,
};
use crate::i18n::text as t;
use crate::render::ansi_clip::ansi_visible_width;
use crate::render::panel_palette::{fg, tint, PanelPalette};

/// 【后台任务】【控制热区】记录单个按钮在面板内行号与起止列，用于精确鼠标命中。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct RibbonItem {
    pub start: usize,
    pub end: usize,
    pub key: crossterm::event::KeyCode,
}

/// 【后台任务】【筛选栏渲染】渲染固定列位的筛选胶囊，并在宽面板右端附带输出通道徽章。
///
/// 参数:
/// - `manager`: 当前任务管理状态
/// - `running`: 运行中任务数量
/// - `width`: 可用内容宽度
/// - `palette`: 当前面板色板
///
/// 返回:
/// - `(渲染文本, 按钮点击列范围列表)`
pub(super) fn filter_ribbon(
    manager: &TaskManager,
    running: usize,
    width: usize,
    palette: &PanelPalette,
) -> (String, Vec<RibbonItem>) {
    let total = manager.tasks.len();
    let options = [
        (Filter::All, t("All", "全部"), total, '1'),
        (Filter::Running, t("Running", "运行中"), running, '2'),
        (
            Filter::Finished,
            t("Finished", "已结束"),
            total.saturating_sub(running),
            '3',
        ),
    ];
    let mut row = String::new();
    let mut buttons = Vec::new();
    let mut col = 0usize;
    for (index, (filter, name, count, hotkey)) in options.iter().enumerate() {
        let label = format!(" {hotkey} {name} {count} ");
        let w = ansi_visible_width(&label);
        if index > 0 && col + w + 1 > width {
            break;
        }
        // 1. 【后台任务】【胶囊高亮】激活筛选项使用焦点底色与强调色，未激活项保持柔和弱化文字
        let segment = if *filter == manager.filter {
            pill(&label, palette.accent, Some(palette.focus_bg), true)
        } else {
            format!("{}{label}{RESET}", fg(palette.muted))
        };
        if manager.confirm.is_none() && !manager.busy {
            buttons.push(RibbonItem {
                start: col,
                end: col + w,
                key: crossterm::event::KeyCode::Char(*hotkey),
            });
        }
        row.push_str(&segment);
        col += w;
        if index + 1 < options.len() && col + 1 <= width {
            row.push(' ');
            col += 1;
        }
    }
    // 2. 【后台任务】【通道徽章】右侧空间充足时展示可点击输出流切换胶囊
    let stream_label = format!(" [v] {} ", stream_name(manager.stream));
    let stream_w = ansi_visible_width(&stream_label);
    if !manager.pager && width >= col + stream_w + 3 {
        let gap = width - col - stream_w;
        row.push_str(&" ".repeat(gap));
        let start = width - stream_w;
        let badge_color = match manager.stream {
            Stream::Stdout => palette.accent,
            Stream::Stderr => palette.removed,
            Stream::All => palette.text,
        };
        row.push_str(&pill(
            &stream_label,
            badge_color,
            Some(palette.hover_bg),
            false,
        ));
        if manager.confirm.is_none() && !manager.busy {
            buttons.push(RibbonItem {
                start,
                end: start + stream_w,
                key: crossterm::event::KeyCode::Char('v'),
            });
        }
    }
    (fit(&row, width), buttons)
}

/// 【后台任务】【任务条目】绘制含状态图标、状态词、任务名称与耗时的对齐行。
///
/// 参数:
/// - `task`: 任务数据
/// - `selected`: 是否为键盘当前选中项
/// - `hovered`: 是否为鼠标悬停项
/// - `width`: 条目栏可用宽度
/// - `palette`: 当前面板色板
///
/// 返回:
/// - 恰好对齐并铺满背景提示的行文本
pub(super) fn task_row(
    task: &Task,
    selected: bool,
    hovered: bool,
    width: usize,
    palette: &PanelPalette,
) -> String {
    let (status, color) = status_style(&task.status, palette);
    let glyph = status_glyph(&task.status);
    let end = if task.status == "running" {
        crate::tools::command::unix_seconds()
    } else {
        task.updated_at
    };
    let elapsed = duration(end.saturating_sub(task.started_at));
    let metadata = if width >= 54 {
        format!("pid {:<6} {elapsed:>7}", task.pid)
    } else {
        format!("{elapsed:>7}")
    };
    let indicator = if selected { "▎" } else { " " };
    let status_pad = 7usize.saturating_sub(ansi_visible_width(status));
    let status_block = format!("{glyph} {status}{}", " ".repeat(status_pad));
    // 1. 【后台任务】【列宽分配】留出左指示条(1) + 空格(1) + 状态(9) + 间隔(2) + 元信息 + 尾空格(1)
    let fixed = 1 + 1 + ansi_visible_width(&status_block) + 2 + metadata.len() + 1;
    let label_width = width.saturating_sub(fixed).max(1);
    let name = if task.label.trim().is_empty() {
        highlight_inline(&task.command, label_width, false, palette)
    } else {
        fit(&safe_text(&task.label), label_width)
    };
    let padding = label_width.saturating_sub(ansi_visible_width(&name));
    let row = format!(
        "{}{}{indicator}{} {}{status_block}\x1b[39m {}{name}\x1b[22m{} {}{metadata}\x1b[39m ",
        fg(if selected {
            palette.accent
        } else {
            palette.border
        }),
        "",
        "",
        fg(color),
        if selected { "\x1b[1m" } else { "" },
        " ".repeat(padding),
        fg(palette.muted),
    );
    // 2. 【后台任务】【视觉反馈】键盘选中行用 focus_bg，悬停行用 hover_bg，透明终端友好
    let fitted = fit(&row, width);
    let padded = if ansi_visible_width(&fitted) < width {
        format!(
            "{fitted}{}",
            " ".repeat(width.saturating_sub(ansi_visible_width(&fitted)))
        )
    } else {
        fitted
    };
    if selected {
        tint(&padded, palette.focus_bg)
    } else if hovered {
        tint(&padded, palette.hover_bg)
    } else {
        padded
    }
}
