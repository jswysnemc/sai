//! 后台任务详情检视器与高对比确认卡片：结构化展示选中任务元数据或待确认目标。

use super::{
    model::{safe_text, TaskManager},
    service::Request,
    task_style::status_style,
    theme::{fit, pill, section_header, status_glyph, RESET},
    view::duration,
};
use crate::i18n::text as t;
use crate::render::ansi_clip::ansi_visible_width;
use crate::render::panel_palette::{fg, PanelPalette};

/// 【后台任务】【选中详情】生成结构化详情卡片行（标题分隔 + 关键字段键值对）。
///
/// 参数:
/// - `manager`: 任务管理状态
/// - `width`: 详情栏可用列宽
/// - `max_rows`: 允许的最大行数
/// - `palette`: 当前面板色板
///
/// 返回:
/// - 长度不超过 `max_rows` 的样式化详情行
pub(super) fn detail_rows(
    manager: &TaskManager,
    width: usize,
    max_rows: usize,
    palette: &PanelPalette,
) -> Vec<String> {
    if max_rows == 0 {
        return Vec::new();
    }
    let Some(task) = manager.selected() else {
        let mut lines = vec![section_header(
            &format!("\x1b[1m{}{}", fg(palette.muted), t("Details", "选中详情")),
            "",
            width,
            palette,
        )];
        if max_rows > 1 {
            lines.push(format!(
                " {}{}{}",
                fg(palette.muted),
                t("No selected task", "无选中任务"),
                RESET
            ));
        }
        lines.resize(max_rows, String::new());
        return lines;
    };
    let (status_label, status_color) = status_style(&task.status, palette);
    let glyph = status_glyph(&task.status);
    let badge = pill(
        &format!(" {glyph} {status_label} "),
        status_color,
        Some(palette.focus_bg),
        true,
    );
    let mut lines = vec![section_header(
        &format!("\x1b[1m{}{}", fg(palette.muted), t("Details", "选中详情")),
        &badge,
        width,
        palette,
    )];
    let end = if task.status == "running" {
        crate::tools::command::unix_seconds()
    } else {
        task.updated_at
    };
    let elapsed = duration(end.saturating_sub(task.started_at));
    let timeout = if task.timeout_seconds == 0 {
        t("unlimited", "无限制").into()
    } else {
        format!("{}s", task.timeout_seconds)
    };
    let identity = format!(
        "{} {}· PID {} · {} {} · {} {timeout}\x1b[39m",
        safe_text(&task.id),
        fg(palette.muted),
        task.pid,
        t("Elapsed", "耗时"),
        elapsed,
        t("Timeout", "超时"),
    );
    let fields = [
        (t("ID", "标识"), identity),
        (t("Directory", "目录"), safe_text(&task.cwd)),
        (t("Command", "命令"), safe_text(&task.command)),
    ];
    for (name, value) in fields {
        if lines.len() >= max_rows {
            break;
        }
        lines.push(field_row(name, &value, width, palette));
    }
    lines.resize(max_rows, String::new());
    lines
}

/// 【后台任务】【危险确认卡】冻结目标后展示醒目警告标题、目标摘要与即时说明。
///
/// 参数:
/// - `request`: 已固定的待确认请求
/// - `manager`: 任务管理状态
/// - `width`: 可用列宽
/// - `max_rows`: 最大可用行数
/// - `palette`: 当前面板色板
///
/// 返回:
/// - 长度填充至 `max_rows` 的确认卡片行
pub(super) fn confirm_rows(
    request: &Request,
    manager: &TaskManager,
    width: usize,
    max_rows: usize,
    palette: &PanelPalette,
) -> Vec<String> {
    if max_rows == 0 {
        return Vec::new();
    }
    let (title, fields) = match request {
        Request::Stop { id, force } => {
            let action = if *force {
                t("Force stop", "强制停止")
            } else {
                t("Stop", "停止")
            };
            let mut rows = Vec::new();
            if let Some(task) = manager.tasks.iter().find(|task| &task.id == id) {
                if !task.label.trim().is_empty() && task.label != task.command {
                    rows.push((t("Task", "任务"), safe_text(&task.label)));
                }
                rows.push((t("Command", "命令"), safe_text(&task.command)));
                rows.push(("PID", task.pid.to_string()));
            }
            (format!("! {action} {}", safe_text(id)), rows)
        }
        Request::Cleanup => {
            let finished = manager
                .tasks
                .iter()
                .filter(|task| task.status != "running")
                .count();
            (
                format!(
                    "! {}",
                    t(
                        "Clean finished records; keep logs",
                        "清理已结束记录，保留日志"
                    )
                ),
                vec![(t("Records", "记录"), finished.to_string())],
            )
        }
        Request::Refresh => (String::new(), Vec::new()),
    };
    let label = format!("\x1b[1m{}{title}", fg(palette.removed));
    let mut result = vec![section_header(&label, "", width, palette)];
    for (name, value) in fields {
        if result.len() >= max_rows {
            break;
        }
        result.push(field_row(name, &value, width, palette));
    }
    result.resize(max_rows, String::new());
    result
}

/// 【后台任务】【字段对齐】统一点对点标签列宽并安全裁剪长路径/命令内容。
///
/// 参数:
/// - `name`: 字段名
/// - `value`: 字段内容
/// - `width`: 最大行宽
/// - `palette`: 当前面板色板
///
/// 返回:
/// - 单行字段文本
fn field_row(name: &str, value: &str, width: usize, palette: &PanelPalette) -> String {
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
    let prefix = format!(" {}{name}{}\x1b[39m  ", fg(palette.muted), " ".repeat(gap));
    let body_budget = width.saturating_sub(ansi_visible_width(&prefix));
    format!("{prefix}{}{RESET}", fit(value, body_budget))
}
