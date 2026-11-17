//! 后台任务详情检视器与高对比确认卡片：结构化展示选中任务元数据、着色命令或待确认目标。

use super::{
    command_style::{highlight_expanded_lines, highlight_inline, needs_expansion},
    model::{safe_text, TaskManager},
    service::Request,
    task_style::status_style,
    theme::{fit, pill, section_header, status_glyph, RESET},
    view::duration,
};
use crate::i18n::text as t;
use crate::render::ansi_clip::ansi_visible_width;
use crate::render::panel_palette::{fg, PanelPalette};
use crossterm::event::KeyCode;

/// 【后台任务】【详情渲染结果】包含详情区视觉行、相对详情左上角的点击热区及命令分页参数。
pub(super) struct DetailRender {
    pub lines: Vec<String>,
    pub hits: Vec<(usize, usize, usize, KeyCode)>,
    pub cmd_visible_rows: usize,
    pub cmd_total_rows: usize,
}

/// 【后台任务】【命令列宽】计算详情区扣除字段标签与间距后留给命令正文的可见列数。
///
/// 参数:
/// - `width`: 详情区域总列宽
///
/// 返回:
/// - 命令正文可用列宽
pub(super) fn command_body_width(width: usize) -> usize {
    width.saturating_sub(field_prefix_width())
}

/// 【后台任务】【展开行数预估】计算指定命令在当前详情列宽下展开后的折行总行数。
///
/// 参数:
/// - `command`: 原始命令字符串
/// - `width`: 详情区域总列宽
/// - `palette`: 当前面板色板
///
/// 返回:
/// - 至少为 1 的命令显示行数
pub(super) fn expanded_command_row_count(
    command: &str,
    width: usize,
    palette: &PanelPalette,
) -> usize {
    highlight_expanded_lines(command, command_body_width(width), palette)
        .len()
        .max(1)
}

/// 【后台任务】【选中详情】生成结构化详情卡片行、展开切换胶囊及命令点击热区。
///
/// 参数:
/// - `manager`: 任务管理状态
/// - `width`: 详情栏可用列宽
/// - `max_rows`: 允许的最大行数
/// - `palette`: 当前面板色板
///
/// 返回:
/// - 含视觉行、可点击热区与命令滚动元信息的详情渲染结果
pub(super) fn detail_rows(
    manager: &TaskManager,
    width: usize,
    max_rows: usize,
    palette: &PanelPalette,
) -> DetailRender {
    if max_rows == 0 {
        return DetailRender {
            lines: Vec::new(),
            hits: Vec::new(),
            cmd_visible_rows: 0,
            cmd_total_rows: 0,
        };
    }
    let Some(task) = manager.selected() else {
        let mut lines = vec![section_header(
            &format!("\x1b[1m{}{}", fg(palette.muted), t("Details", "选中详情")),
            "",
            width,
            palette,
        )];
        if max_rows > 1 {
            lines.push(fit(
                &format!(
                    " {}{}{}",
                    fg(palette.muted),
                    t("No selected task", "无选中任务"),
                    RESET
                ),
                width,
            ));
        }
        lines.resize(max_rows, String::new());
        return DetailRender {
            lines,
            hits: Vec::new(),
            cmd_visible_rows: 0,
            cmd_total_rows: 0,
        };
    };

    // 1. 【后台任务】【命令预算】预先计算单行可展开状态或展开态折行结果与钳制后的滚动偏移
    let body_budget = command_body_width(width);
    let expandable = needs_expansion(&task.command, body_budget);
    let (expanded_lines, cmd_visible_rows, cmd_total_rows, cmd_offset) =
        if manager.command_expanded {
            let wrapped = highlight_expanded_lines(&task.command, body_budget, palette);
            let visible = max_rows.saturating_sub(3);
            let total = wrapped.len();
            let offset = manager.command_offset.min(total.saturating_sub(visible));
            (wrapped, visible, total, offset)
        } else {
            (Vec::new(), usize::from(max_rows > 3), 0, 0)
        };

    // 2. 【后台任务】【标题与胶囊】构建带状态徽章和 [c] 展开/收起切换按钮的详情标题行
    let mut hits = Vec::new();
    let (header, pill_hit) = detail_header(
        &task.status,
        manager.command_expanded,
        expandable,
        cmd_offset,
        cmd_visible_rows,
        cmd_total_rows,
        width,
        palette,
    );
    if let Some((start, end)) = pill_hit {
        hits.push((0, start, end, KeyCode::Char('c')));
    }
    let mut lines = vec![header];

    // 3. 【后台任务】【基础元数据】填充任务标识、进程耗时及工作目录行
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
    for (name, value) in [
        (t("ID", "标识"), identity),
        (t("Directory", "目录"), safe_text(&task.cwd)),
    ] {
        if lines.len() >= max_rows {
            break;
        }
        lines.push(field_row(name, &value, width, palette));
    }

    // 4. 【后台任务】【命令展示】折叠态渲染单行语法着色预览，展开态按偏移渲染对齐多行命令
    if lines.len() < max_rows {
        if !manager.command_expanded {
            let styled = highlight_inline(&task.command, body_budget, true, palette);
            let row = field_row(t("Command", "命令"), &styled, width, palette);
            let row_w = ansi_visible_width(&row).min(width);
            if row_w > 1 {
                hits.push((3, 1, row_w, KeyCode::Char('c')));
            }
            lines.push(row);
        } else {
            let indent = " ".repeat(field_prefix_width().min(width));
            for (idx, cmd_line) in expanded_lines
                .iter()
                .skip(cmd_offset)
                .take(cmd_visible_rows)
                .enumerate()
            {
                if idx == 0 {
                    let row = field_row(t("Command", "命令"), cmd_line, width, palette);
                    let row_w = ansi_visible_width(&row).min(width);
                    if row_w > 1 {
                        hits.push((3, 1, row_w, KeyCode::Char('c')));
                    }
                    lines.push(row);
                } else {
                    lines.push(fit(&format!("{indent}{cmd_line}{RESET}"), width));
                }
            }
        }
    }

    lines.resize(max_rows, String::new());
    DetailRender {
        lines,
        hits,
        cmd_visible_rows,
        cmd_total_rows,
    }
}

/// 【后台任务】【详情标题栏】组合右侧命令展开胶囊与任务状态徽章，并返回切换胶囊的精确列区间。
///
/// 参数:
/// - `status`: 任务状态字符串
/// - `expanded`: 当前是否已展开完整命令
/// - `expandable`: 当前命令是否含换行或超出单行预算
/// - `offset`: 展开态首行滚动偏移
/// - `visible_rows`: 展开态可见行数
/// - `total_rows`: 展开态总行数
/// - `width`: 详情栏总宽度
/// - `palette`: 当前面板色板
///
/// 返回:
/// - `(标题分隔行文本, 可选的切换胶囊起止列)`
#[allow(clippy::too_many_arguments)]
fn detail_header(
    status: &str,
    expanded: bool,
    expandable: bool,
    offset: usize,
    visible_rows: usize,
    total_rows: usize,
    width: usize,
    palette: &PanelPalette,
) -> (String, Option<(usize, usize)>) {
    let label = format!("\x1b[1m{}{}", fg(palette.muted), t("Details", "选中详情"));
    let label_w = ansi_visible_width(&label);
    let (status_label, status_color) = status_style(status, palette);
    let glyph = status_glyph(status);
    let badge = pill(
        &format!(" {glyph} {status_label} "),
        status_color,
        Some(palette.focus_bg),
        true,
    );
    let badge_w = ansi_visible_width(&badge);

    // 1. 【后台任务】【候选胶囊】按宽度自适应挑选完整文案、带页码文案或紧凑按键胶囊
    let candidates = if expanded {
        if total_rows > visible_rows && visible_rows > 0 {
            let first = offset + 1;
            let last = (offset + visible_rows).min(total_rows);
            vec![
                format!(
                    " ▾ [c] {} ({first}-{last}/{total_rows}) ",
                    t("Collapse cmd", "收起命令")
                ),
                format!(" ▾ [c] ({first}-{last}/{total_rows}) "),
                " ▾ [c] ".to_string(),
            ]
        } else {
            vec![
                format!(" ▾ [c] {} ", t("Collapse cmd", "收起命令")),
                " ▾ [c] ".to_string(),
            ]
        }
    } else {
        vec![
            format!(" ▸ [c] {} ", t("Expand cmd", "展开命令")),
            " ▸ [c] ".to_string(),
        ]
    };

    let (pill_fg, pill_bg, pill_bold) = if expanded {
        (palette.accent, Some(palette.focus_bg), true)
    } else if expandable {
        (palette.accent, Some(palette.hover_bg), false)
    } else {
        (palette.muted, Some(palette.hover_bg), false)
    };

    // 2. 【后台任务】【列位对齐】与 section_header 保持完全一致的留白计算以导出精确热区
    for text in candidates {
        let cmd_w = ansi_visible_width(&text);
        let right_w = cmd_w + 1 + badge_w;
        if label_w + right_w + 4 <= width {
            let cmd_pill = pill(&text, pill_fg, pill_bg, pill_bold);
            let right = format!("{cmd_pill} {badge}");
            let gap = width.saturating_sub(label_w + right_w + 5);
            let start = 3 + label_w + gap;
            return (
                section_header(&label, &right, width, palette),
                Some((start, start + cmd_w)),
            );
        }
    }

    (section_header(&label, &badge, width, palette), None)
}

/// 【后台任务】【危险确认卡】冻结目标后展示醒目警告标题、着色命令摘要与即时说明。
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
                let cmd_budget = command_body_width(width);
                rows.push((
                    t("Command", "命令"),
                    highlight_inline(&task.command, cmd_budget, true, palette),
                ));
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

/// 【后台任务】【标签列宽】计算详情与确认卡字段名的统一对齐宽度。
///
/// 参数:
/// - 无
///
/// 返回:
/// - 字段名称列的最大可见列数
fn field_label_column() -> usize {
    [
        t("ID", "标识"),
        t("Directory", "目录"),
        t("Command", "命令"),
        t("Task", "任务"),
        t("Records", "记录"),
    ]
    .iter()
    .map(|label| ansi_visible_width(label))
    .max()
    .unwrap_or(0)
}

/// 【后台任务】【前缀宽度】返回字段列左留白、对齐字段名与双空格分隔占用的总列数。
///
/// 参数:
/// - 无
///
/// 返回:
/// - 字段值起始列偏移
fn field_prefix_width() -> usize {
    1 + field_label_column() + 2
}

/// 【后台任务】【字段对齐】统一点对点标签列宽并安全裁剪长路径或着色命令内容。
///
/// 参数:
/// - `name`: 字段名
/// - `value`: 字段内容（允许包含闭合良好的 ANSI 着色序列）
/// - `width`: 最大行宽
/// - `palette`: 当前面板色板
///
/// 返回:
/// - 单行字段文本
fn field_row(name: &str, value: &str, width: usize, palette: &PanelPalette) -> String {
    let column = field_label_column();
    let gap = column.saturating_sub(ansi_visible_width(name));
    let prefix = format!(" {}{name}{}\x1b[39m  ", fg(palette.muted), " ".repeat(gap));
    let body_budget = width.saturating_sub(ansi_visible_width(&prefix));
    fit(&format!("{prefix}{}{RESET}", fit(value, body_budget)), width)
}
