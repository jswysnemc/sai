//! 后台任务列表视图的行组件：筛选标签、任务行、分组标题、选中详情与停止确认块。

use super::{
    model::{safe_text, Filter, Task, TaskManager},
    service::Request,
    task_style::status_style,
    view::{duration, fit},
};
use crate::i18n::text as t;
use crate::render::ansi_clip::ansi_visible_width;
use crate::render::panel_palette::{fg, tint, PanelPalette};

/// 详情区固定行数，无选择状态也补齐到该高度，切换时输出区位置不变
pub(super) const DETAIL_ROWS: usize = 3;

/// 【后台任务】【筛选标签】三个范围宽度恒定，当前范围用强调色加浅底标出，切换时文字不位移。
/// 参数: manager 为面板状态，running 为运行中数量，palette 为主题；返回: 筛选行。
pub(super) fn filters(manager: &TaskManager, running: usize, palette: &PanelPalette) -> String {
    let total = manager.tasks.len();
    let options = [
        (Filter::All, t("All", "全部"), total),
        (Filter::Running, t("Running", "运行中"), running),
        (Filter::Finished, t("Finished", "已结束"), total - running),
    ];
    let mut row = String::new();
    for (index, (filter, name, count)) in options.iter().enumerate() {
        let label = format!(" {} {name} {count} ", index + 1);
        // 1. 【后台任务】【当前范围】浅底只覆盖标签本身，未选范围保持弱化色
        if *filter == manager.filter {
            row.push_str(&tint(
                &format!("\x1b[1m{}{label}", fg(palette.accent)),
                palette.focus_bg,
            ));
        } else {
            row.push_str(&format!("{}{label}\x1b[0m", fg(palette.muted)));
        }
        row.push(' ');
    }
    row
}

/// 【后台任务】【单行条目】状态词、名称、PID 与耗时分列对齐；选中使用焦点底色，悬停使用浅底。
/// 参数: task 为条目，selected 为选中状态，hovered 为悬停状态，width 为内容宽度，palette 为主题；返回: 铺满宽度的样式行。
pub(super) fn task_row(
    task: &Task,
    selected: bool,
    hovered: bool,
    width: usize,
    palette: &PanelPalette,
) -> String {
    let (status, color) = status_style(&task.status, palette);
    let end = if task.status == "running" {
        crate::tools::command::unix_seconds()
    } else {
        task.updated_at
    };
    let elapsed = duration(end.saturating_sub(task.started_at));
    let metadata = if width >= 55 {
        format!("pid {:<7} {elapsed:>8}", task.pid)
    } else {
        format!("{elapsed:>8}")
    };
    // 1. 【后台任务】【列宽分配】首尾各留一列，状态列固定七列，名称占用剩余宽度
    let label_width = width.saturating_sub(11 + metadata.len());
    let label = if task.label.trim().is_empty() {
        &task.command
    } else {
        &task.label
    };
    let name = fit(&safe_text(label), label_width);
    let padding = label_width.saturating_sub(ansi_visible_width(&name));
    let row = format!(
        " {}{status:<7}\x1b[39m {}{name}\x1b[22m{} {}{metadata}\x1b[39m ",
        fg(color),
        if selected { "\x1b[1m" } else { "" },
        " ".repeat(padding),
        fg(palette.muted),
    );
    // 2. 【后台任务】【状态叠加】键盘选择比鼠标悬停深一级，二者同时成立时以选择为准
    if selected {
        tint(&row, palette.focus_bg)
    } else if hovered {
        tint(&row, palette.hover_bg)
    } else {
        row
    }
}

/// 【后台任务】【分组标题】标题文字在前、细线补齐到右边缘，与侧栏分组标题一致。
/// 参数: label 为已着色标题，width 为内容宽度，palette 为主题；返回: 标题行。
pub(super) fn heading(label: &str, width: usize, palette: &PanelPalette) -> String {
    let used = ansi_visible_width(label) + 2;
    format!(
        " {label}\x1b[0m {}{}\x1b[0m",
        fg(palette.border),
        "─".repeat(width.saturating_sub(used))
    )
}

/// 【后台任务】【内联详情】标签减淡并统一列宽，正文保留默认色；输出另由尾部输出区显示。
/// 参数: manager 为面板状态，palette 为主题；返回: 固定行数的详情。
pub(super) fn detail_rows(manager: &TaskManager, palette: &PanelPalette) -> Vec<String> {
    let Some(task) = manager.selected() else {
        return padded(vec![format!(
            " {}{}\x1b[0m",
            fg(palette.muted),
            t("No selected task", "无选中任务")
        )]);
    };
    let timeout = if task.timeout_seconds == 0 {
        t("unlimited", "无限制").into()
    } else {
        format!("{}s", task.timeout_seconds)
    };
    let identity = format!(
        "{} {}· PID {} · {}: {timeout}\x1b[39m",
        safe_text(&task.id),
        fg(palette.muted),
        task.pid,
        t("Timeout", "超时")
    );
    let fields = [
        (t("ID", "标识"), identity),
        (t("Directory", "目录"), safe_text(&task.cwd)),
        (t("Command", "命令"), safe_text(&task.command)),
    ];
    padded(
        fields
            .iter()
            .map(|(name, value)| field(name, value, palette))
            .collect(),
    )
}

/// 【后台任务】【停止确认】保留任务列表，把详情与输出区换成带警示色标题的目标信息，确认目标不随刷新变化。
/// 参数: request 为待确认操作，manager 为面板状态，width 为内容宽度，palette 为主题；返回: 标题行与固定行数的目标信息。
pub(super) fn confirm_rows(
    request: &Request,
    manager: &TaskManager,
    width: usize,
    palette: &PanelPalette,
) -> Vec<String> {
    let (title, rows) = match request {
        Request::Stop { id, force } => {
            let action = if *force {
                t("Force stop", "强制停止")
            } else {
                t("Stop", "停止")
            };
            let mut rows = Vec::new();
            if let Some(task) = manager.tasks.iter().find(|task| &task.id == id) {
                if !task.label.trim().is_empty() && task.label != task.command {
                    rows.push(field(t("Task", "任务"), &safe_text(&task.label), palette));
                }
                rows.push(field(
                    t("Command", "命令"),
                    &safe_text(&task.command),
                    palette,
                ));
                rows.push(field("PID", &task.pid.to_string(), palette));
            }
            (format!("{action} {}", safe_text(id)), rows)
        }
        Request::Cleanup => {
            let finished = manager
                .tasks
                .iter()
                .filter(|task| task.status != "running")
                .count();
            (
                t(
                    "Clean finished records; keep logs",
                    "清理已结束记录，保留日志",
                )
                .into(),
                vec![field(t("Records", "记录"), &finished.to_string(), palette)],
            )
        }
        Request::Refresh => (String::new(), Vec::new()),
    };
    let label = format!("\x1b[1m{}{title}", fg(palette.removed));
    let mut result = vec![heading(&label, width, palette)];
    result.extend(padded(rows));
    result
}

/// 【后台任务】【字段对齐】按当前语言最长标签统一列宽，标签减淡、内容保持默认色。
/// 参数: name 为标签，value 为已着色内容，palette 为主题；返回: 字段行。
fn field(name: &str, value: &str, palette: &PanelPalette) -> String {
    let column = [
        t("ID", "标识"),
        t("Directory", "目录"),
        t("Command", "命令"),
        t("Task", "任务"),
        t("Records", "记录"),
    ]
    .iter()
    .map(|label| ansi_visible_width(label))
    .max()
    .unwrap_or(0);
    let gap = column.saturating_sub(ansi_visible_width(name));
    format!(
        " {}{name}{}\x1b[39m  {value}",
        fg(palette.muted),
        " ".repeat(gap)
    )
}

/// 【后台任务】【固定高度】把详情或确认内容补齐到固定行数，超出部分舍弃。
/// 参数: rows 为内容行；返回: 恰好 DETAIL_ROWS 行。
fn padded(mut rows: Vec<String>) -> Vec<String> {
    rows.resize(DETAIL_ROWS, String::new());
    rows
}
