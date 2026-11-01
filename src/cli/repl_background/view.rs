//! 紧凑圆角任务面板：状态列表、内联详情与边框操作提示。

use super::{
    model::{safe_text, Filter, Stream, Task, TaskManager},
    service::Request,
};
use crate::config_tui::theme::{BOLD, BRAND, DANGER, DIM, MUTED, OK, RESET};
use crate::i18n::text as t;
use crate::render::ansi_clip::{ansi_visible_width, clip_ansi_to_width};
use std::time::Instant;

pub(super) struct TaskFrame {
    pub lines: Vec<String>,
    pub targets: Vec<(u16, String)>,
    pub page_rows: usize,
    pub x: u16,
    pub width: u16,
}

/// 【后台任务】【面板组装】按真实终端大小绘制紧凑单体面板，窄终端不越界。
/// 参数: manager 为状态，size 为列数和行数；返回: 帧内容与点击区域。
pub(super) fn render(manager: &mut TaskManager, size: (u16, u16)) -> TaskFrame {
    let (cols, rows) = size;
    let width = cols.saturating_sub(2).min(90).max(1).min(cols);
    let x = cols.saturating_sub(width) / 2;
    let inner = usize::from(width.saturating_sub(4));
    let height = rows.min(if manager.pager { 18 } else { 16 });
    let y = rows.saturating_sub(height).min(2);
    let page_rows = usize::from(height.saturating_sub(if manager.pager { 4 } else { 9 })).max(1);
    let mut targets = Vec::new();
    let mut body = Vec::new();
    let title;
    if manager.pager {
        let id = manager.selected_id.as_deref().unwrap_or_default();
        title = format!(
            "{BOLD}{}: {}{RESET} {BRAND}[{}]{RESET} {}",
            t("Task logs", "任务日志"),
            safe_text(id),
            stream_name(manager.stream),
            if manager.follow {
                t("Follow on", "跟随开启")
            } else {
                t("Follow off", "跟随暂停")
            }
        );
        body = super::pager_view::rows(manager, inner, page_rows);
    } else {
        let running = manager
            .tasks
            .iter()
            .filter(|task| task.status == "running")
            .count();
        title = format!(
            "{BRAND}◆ {BOLD}{}{RESET} {MUTED}{running}/{} {}{RESET}",
            t("Background tasks", "后台任务管理"),
            manager.tasks.len(),
            t("running", "运行中")
        );
        let filters = [
            (Filter::All, t("All", "全部"), manager.tasks.len()),
            (Filter::Running, t("Running", "运行中"), running),
            (
                Filter::Finished,
                t("Finished", "已结束"),
                manager.tasks.len() - running,
            ),
        ];
        body.push(
            filters
                .iter()
                .enumerate()
                .map(|(index, (filter, name, count))| {
                    if *filter == manager.filter {
                        format!("{BRAND}[{} {name} {count}]{RESET}", index + 1)
                    } else {
                        format!("{MUTED}{} {name} {count}{RESET}", index + 1)
                    }
                })
                .collect::<Vec<_>>()
                .join("  "),
        );
        body.push(format!("{DIM}{}{RESET}", "─".repeat(inner)));
        let visible = manager.visible();
        let selected = visible
            .iter()
            .position(|index| Some(&manager.tasks[*index].id) == manager.selected_id.as_ref())
            .unwrap_or(0);
        let start = selected
            .saturating_sub(page_rows - 1)
            .min(visible.len().saturating_sub(page_rows));
        for index in visible.iter().skip(start).take(page_rows) {
            let task = &manager.tasks[*index];
            targets.push((y + 1 + body.len() as u16, task.id.clone()));
            body.push(task_row(
                task,
                Some(&task.id) == manager.selected_id.as_ref(),
                inner,
            ));
        }
        if visible.is_empty() {
            body.push(format!(
                "{MUTED}{}{RESET}",
                if manager.loading {
                    t("Loading tasks...", "正在获取任务…")
                } else {
                    t("No matching tasks", "暂无匹配任务")
                }
            ));
        }
        while body.len() < page_rows + 2 {
            body.push(String::new());
        }
        body.push(format!(
            "{DIM}── {} {}{RESET}",
            t("Details", "选中详情"),
            "─".repeat(inner.saturating_sub(14))
        ));
        body.extend(detail_rows(manager));
    }
    if let Some(request) = &manager.confirm {
        // 1. 【后台任务】【目标确认】确认内容保存任务标识，不随列表刷新改变目标
        let message = match request {
            Request::Stop { id, force } => format!(
                "{} {}",
                if *force {
                    t("Force stop", "强制停止")
                } else {
                    t("Stop", "停止")
                },
                safe_text(id)
            ),
            Request::Cleanup => t(
                "Clean finished records; keep logs",
                "清理已结束记录，保留日志",
            )
            .into(),
            Request::Refresh => String::new(),
        };
        body.clear();
        targets.clear();
        body.push(format!("{DANGER}{message}{RESET}"));
        if let Request::Stop { id, .. } = request {
            if let Some(task) = manager.tasks.iter().find(|task| &task.id == id) {
                body.push(safe_text(&task.label));
                body.push(safe_text(&task.command));
            }
        }
    }
    let footer = footer(manager, inner);
    let mut panel = vec![border('╭', '╮', &title, width)];
    panel.extend(
        body.into_iter()
            .take(usize::from(height.saturating_sub(2)))
            .map(|line| boxed(&line, width)),
    );
    if height >= 2 {
        panel.push(border('╰', '╯', &footer, width));
    }
    panel.truncate(usize::from(rows));
    let mut lines = vec![String::new(); usize::from(y)];
    lines.extend(
        panel
            .into_iter()
            .map(|line| format!("{}{line}", " ".repeat(usize::from(x)))),
    );
    TaskFrame {
        lines,
        targets,
        page_rows,
        x,
        width,
    }
}

/// 【后台任务】【单行条目】按列宽排列状态、名称、PID 与相对耗时。
/// 参数: task 为条目，selected 为选中状态，width 为内容宽度；返回: 样式文本。
fn task_row(task: &Task, selected: bool, width: usize) -> String {
    let (tag, color) = match task.status.as_str() {
        "running" => ("RUN", OK),
        "exited" => ("EXIT", MUTED),
        "stopped" => ("STOP", BRAND),
        "timed_out" => ("TIME", DANGER),
        "failed" => ("FAIL", DANGER),
        _ => ("?", MUTED),
    };
    let end = if task.status == "running" {
        crate::tools::command::unix_seconds()
    } else {
        task.updated_at
    };
    let duration = duration(end.saturating_sub(task.started_at));
    let metadata = if width >= 55 {
        format!("pid {:<7} {duration:>8}", task.pid)
    } else {
        format!("{duration:>8}")
    };
    let label_width = width.saturating_sub(11 + metadata.len());
    let name = fit(
        &safe_text(if task.label.is_empty() {
            &task.command
        } else {
            &task.label
        }),
        label_width,
    );
    let padding = label_width.saturating_sub(ansi_visible_width(&name));
    format!(
        "{} {color}[{tag:<4}]{RESET} {}{name}{RESET}{} {MUTED}{metadata}{RESET}",
        if selected { ">" } else { " " },
        if selected { BOLD } else { "" },
        " ".repeat(padding)
    )
}

/// 【后台任务】【内联详情】提供标识、目录、命令及当前流末行预览。
/// 参数: manager 为任务状态；返回: 四行紧凑详情。
fn detail_rows(manager: &TaskManager) -> Vec<String> {
    let Some(task) = manager.selected() else {
        return vec![t("No selected task", "无选中任务").into()];
    };
    let timeout = if task.timeout_seconds == 0 {
        t("unlimited", "无限制").into()
    } else {
        format!("{}s", task.timeout_seconds)
    };
    let preview = if manager.output_id != manager.selected_id {
        t("Loading output...", "正在读取输出…").into()
    } else if let Some(error) = &manager.output_error {
        format!("{}: {}", t("Read failed", "读取失败"), safe_text(error))
    } else {
        manager
            .log_lines()
            .last()
            .cloned()
            .unwrap_or_else(|| t("No output", "暂无输出").into())
    };
    vec![
        format!(
            "ID: {} · PID {} · {}: {timeout}",
            safe_text(&task.id),
            task.pid,
            t("Timeout", "超时")
        ),
        format!("{}: {}", t("Directory", "目录"), safe_text(&task.cwd)),
        format!("{}: {}", t("Command", "命令"), safe_text(&task.command)),
        format!("{MUTED}[{}] {preview}{RESET}", stream_name(manager.stream)),
    ]
}

/// 【后台任务】【页脚提示】显示当前状态下可用的按键与瞬态反馈。
/// 参数: manager 为状态，width 为可用宽度；返回: 底框文本。
fn footer(manager: &TaskManager, width: usize) -> String {
    if manager.confirm.is_some() {
        return t(
            "Enter/y confirm · Esc/n cancel",
            "Enter/y 确认 · Esc/n 取消",
        )
        .into();
    }
    if manager.busy {
        return t("Working...", "正在执行操作…").into();
    }
    if let Some((text, error, expiry)) = &manager.notice {
        if *error || Instant::now() < *expiry {
            return format!("{}{text}{RESET}", if *error { DANGER } else { OK });
        }
    }
    if manager.pager {
        if width < 55 {
            t(
                "Esc back · Tab stream · f follow · arrows scroll",
                "Esc 返回 · Tab 通道 · f 跟随 · ↑↓ 滚动",
            )
            .into()
        } else {
            t(
                "Esc back · Tab stream · f follow · PgUp/PgDn page · g/G first/last",
                "Esc 返回 · Tab 通道 · f 跟随 · PgUp/PgDn 翻页 · g/G 首尾",
            )
            .into()
        }
    } else if width < 55 {
        t(
            "Esc back · Enter logs · Tab filter · s/f stop · x clean",
            "Esc 返回 · Enter 日志 · Tab 筛选 · s/f 停止 · x 清理",
        )
        .into()
    } else {
        t(
            "Esc back · Enter logs · Tab filter · s stop · f force · x clean · r refresh",
            "Esc 返回 · Enter 日志 · Tab 筛选 · s 停止 · f 强制 · x 清理 · r 刷新",
        )
        .into()
    }
}

/// 【后台任务】【流名称】返回稳定输出通道标记。
/// 参数: stream 为输出流；返回: 通道名。
pub(super) fn stream_name(stream: Stream) -> &'static str {
    match stream {
        Stream::Stdout => "STDOUT",
        Stream::Stderr => "STDERR",
        Stream::All => "ALL",
    }
}
/// 【后台任务】【相对时长】将秒数格式化为紧凑时间。
/// 参数: seconds 为秒数；返回: 时长。
pub(super) fn duration(seconds: u64) -> String {
    if seconds >= 3600 {
        format!("{}h {}m", seconds / 3600, seconds % 3600 / 60)
    } else if seconds >= 60 {
        format!("{}m {}s", seconds / 60, seconds % 60)
    } else {
        format!("{seconds}s")
    }
}
/// 【后台任务】【宽度约束】仅超宽时截断，保留可信样式。
/// 参数: text 为文本，width 为列宽；返回: 不超宽的文本。
pub(super) fn fit(text: &str, width: usize) -> String {
    if ansi_visible_width(text) <= width {
        text.into()
    } else {
        clip_ansi_to_width(text, width, if width > 0 { "…" } else { "" })
    }
}
/// 【后台任务】【边框绘制】生成带嵌入信息的圆角边框。
/// 参数: left/right 为边角，label 为可信样式文本，width 为外框宽度；返回: 边框行。
fn border(left: char, right: char, label: &str, width: u16) -> String {
    if width < 2 {
        return fit(&left.to_string(), usize::from(width));
    }
    let label = fit(&format!(" {label} "), usize::from(width - 2));
    format!(
        "{DIM}{left}{RESET}{label}{DIM}{}{right}{RESET}",
        "─".repeat(usize::from(width - 2).saturating_sub(ansi_visible_width(&label)))
    )
}
/// 【后台任务】【内容边框】补齐内容行并保持外框宽度。
/// 参数: text 为可信样式文本，width 为外框宽度；返回: 带边框内容。
fn boxed(text: &str, width: u16) -> String {
    if width < 4 {
        return fit("││", usize::from(width));
    }
    let inner = usize::from(width - 4);
    let text = fit(text, inner);
    format!(
        "{DIM}│{RESET} {text}{RESET}{} {DIM}│{RESET}",
        " ".repeat(inner.saturating_sub(ansi_visible_width(&text)))
    )
}
