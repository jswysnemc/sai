//! 后台任务列表模式正文排版：在宽屏下采用左右双栏、在窄屏下采用上下堆叠，确认时固定任务列表。

use super::{
    inspector_view::{confirm_rows, detail_rows},
    model::TaskManager,
    pager_view::tail_rows,
    panel_rect::PanelRect,
    split_layout::SplitLayout,
    theme::{fit, section_header, stream_name},
    view_list::{filter_ribbon, task_row, RibbonItem},
};
use crate::i18n::text as t;
use crate::render::ansi_clip::ansi_visible_width;
use crate::render::panel_palette::{fg, PanelPalette};

/// 【后台任务】【列表正文渲染结果】返回正文视觉行、任务视口容量、筛选栏热区与尾部输出区。
pub(super) struct ListBodyRender {
    pub lines: Vec<String>,
    pub task_viewport_rows: usize,
    pub ribbon_hits: Vec<RibbonItem>,
    pub tail_rect: Option<PanelRect>,
    pub tail_rows: usize,
}

/// 【后台任务】【列表正文】组装筛选栏、任务队列、选中详情与尾部输出，宽屏双栏对齐、窄屏紧凑堆叠。
///
/// 参数:
/// - `manager`: 任务管理状态
/// - `inner`: 面板正文净宽
/// - `rows`: 面板正文总行数
/// - `content_x`: 正文左侧终端起始列
/// - `top`: 正文首行对应的终端行号
/// - `targets`: 收集每个任务行对应的终端行号与任务标识
/// - `palette`: 当前面板色板
///
/// 返回:
/// - 列表正文渲染结果
pub(super) fn render(
    manager: &TaskManager,
    inner: usize,
    rows: usize,
    content_x: u16,
    top: u16,
    targets: &mut Vec<(u16, String)>,
    palette: &PanelPalette,
) -> ListBodyRender {
    let running = manager
        .tasks
        .iter()
        .filter(|task| task.status == "running")
        .count();
    let (ribbon, ribbon_hits) = filter_ribbon(manager, running, inner, palette);
    let divider = format!("{}{}\x1b[0m", fg(palette.border), "─".repeat(inner));
    let layout = SplitLayout::compute(inner, rows, manager.tasks.len(), manager.confirm.is_some());

    match layout {
        SplitLayout::Split {
            left_w,
            right_w,
            task_rows,
            detail_rows: detail_cap,
            tail_rows: tail_cap,
        } => {
            // 1. 【后台任务】【左栏队列】左侧列展示带状态条和悬停态的独立任务列表
            let left = build_task_lines(
                manager,
                left_w,
                task_rows,
                top.saturating_add(2),
                targets,
                palette,
            );
            // 2. 【后台任务】【右栏检视】确认期间切为目标确认卡，常规状态显示元数据与尾部日志
            let mut right = if let Some(request) = &manager.confirm {
                targets.clear();
                confirm_rows(request, manager, right_w, task_rows, palette)
            } else {
                let mut panel = detail_rows(manager, right_w, detail_cap, palette);
                if tail_cap > 0 && manager.selected().is_some() {
                    let logs = manager.log_lines();
                    panel.push(section_header(
                        &output_label(manager, logs.len(), palette),
                        "",
                        right_w,
                        palette,
                    ));
                    panel.extend(tail_rows(manager, &logs, right_w, tail_cap, palette));
                }
                panel
            };
            right.resize(task_rows, String::new());
            let mut body = vec![ribbon, divider];
            let sep = format!(" {}│\x1b[0m ", fg(palette.border));
            for index in 0..task_rows {
                let l = fit(left.get(index).map(String::as_str).unwrap_or(""), left_w);
                let l_pad = " ".repeat(left_w.saturating_sub(ansi_visible_width(&l)));
                let r = fit(right.get(index).map(String::as_str).unwrap_or(""), right_w);
                body.push(format!("{l}{l_pad}{sep}{r}"));
            }
            body.resize(rows, String::new());
            let tail_rect =
                (manager.confirm.is_none() && tail_cap > 0 && manager.selected().is_some())
                    .then_some(PanelRect {
                        x: content_x.saturating_add(left_w as u16).saturating_add(3),
                        y: top.saturating_add(2),
                        width: right_w as u16,
                        height: task_rows as u16,
                    });
            ListBodyRender {
                lines: body,
                task_viewport_rows: task_rows,
                ribbon_hits,
                tail_rect,
                tail_rows: tail_cap,
            }
        }
        SplitLayout::Stacked {
            task_rows,
            tail_rows: tail_cap,
        } => {
            // 3. 【后台任务】【窄屏堆叠】窄终端保持纵向分区，不挤占任务标题和路径空间
            let mut body = vec![ribbon, divider];
            let list = build_task_lines(
                manager,
                inner,
                task_rows,
                top.saturating_add(2),
                targets,
                palette,
            );
            body.extend(list);
            let tail_start_row = body.len();
            let tail_visible = tail_cap.saturating_sub(1);
            if let Some(request) = &manager.confirm {
                targets.clear();
                body.extend(confirm_rows(request, manager, inner, 4, palette));
            } else {
                body.extend(detail_rows(manager, inner, 4, palette));
                if tail_cap > 0 && manager.selected().is_some() {
                    let logs = manager.log_lines();
                    body.push(section_header(
                        &output_label(manager, logs.len(), palette),
                        "",
                        inner,
                        palette,
                    ));
                    body.extend(tail_rows(manager, &logs, inner, tail_visible, palette));
                }
            }
            let tail_len = body.len().saturating_sub(tail_start_row);
            body.resize(rows, String::new());
            let tail_rect =
                (manager.confirm.is_none() && tail_visible > 0 && manager.selected().is_some())
                    .then_some(PanelRect {
                        x: content_x,
                        y: top.saturating_add(tail_start_row as u16),
                        width: inner as u16,
                        height: tail_len as u16,
                    });
            ListBodyRender {
                lines: body,
                task_viewport_rows: task_rows,
                ribbon_hits,
                tail_rect,
                tail_rows: tail_visible,
            }
        }
    }
}

/// 【后台任务】【队列构建】按固定窗口高度裁剪可见任务并登记鼠标点击行号。
///
/// 参数:
/// - `manager`: 任务管理状态
/// - `width`: 队列列宽
/// - `capacity`: 任务区可见行数
/// - `start_row`: 队列首行在终端中的行号
/// - `targets`: 输出命中表
/// - `palette`: 当前面板色板
///
/// 返回:
/// - 填充至 `capacity` 行的任务队列视觉行
fn build_task_lines(
    manager: &TaskManager,
    width: usize,
    capacity: usize,
    start_row: u16,
    targets: &mut Vec<(u16, String)>,
    palette: &PanelPalette,
) -> Vec<String> {
    let capacity = capacity.max(1);
    let visible = manager.visible();
    let selected = visible
        .iter()
        .position(|index| Some(&manager.tasks[*index].id) == manager.selected_id.as_ref())
        .unwrap_or(0);
    let start = selected
        .saturating_sub(capacity - 1)
        .min(visible.len().saturating_sub(capacity));
    let mut lines = Vec::with_capacity(capacity);
    for (offset, index) in visible.iter().skip(start).take(capacity).enumerate() {
        let task = &manager.tasks[*index];
        targets.push((start_row.saturating_add(offset as u16), task.id.clone()));
        lines.push(task_row(
            task,
            Some(&task.id) == manager.selected_id.as_ref(),
            manager.confirm.is_none() && Some(&task.id) == manager.hovered.as_ref(),
            width,
            palette,
        ));
    }
    if visible.is_empty() {
        lines.push(format!(
            " {}{}\x1b[0m",
            fg(palette.muted),
            if manager.loading {
                t("Loading tasks...", "正在获取任务…")
            } else {
                t("No matching tasks", "暂无匹配任务")
            }
        ));
    }
    lines.resize(capacity, String::new());
    lines
}

/// 【后台任务】【输出标题】展示当前通道名与缓冲区行数。
///
/// 参数:
/// - `manager`: 任务管理状态
/// - `lines`: 已读取日志总行数
/// - `palette`: 当前面板色板
///
/// 返回:
/// - 着色后的小节标题文字
fn output_label(manager: &TaskManager, lines: usize, palette: &PanelPalette) -> String {
    let loaded = manager.output_id == manager.selected_id && manager.output_error.is_none();
    let count = if loaded {
        format!(" · {lines} {}", t("lines", "行"))
    } else {
        String::new()
    };
    let scroll_mark = if manager.tail_offset > 0 {
        format!(" · ↑{}", manager.tail_offset)
    } else {
        String::new()
    };
    format!(
        "\x1b[1m{}{}\x1b[22m · {}{count}{scroll_mark}",
        fg(palette.muted),
        t("Output", "输出"),
        stream_name(manager.stream),
    )
}
