//! 列表模式正文：按面板高度在任务视口与尾部输出之间分配行数，确认时保留任务列表。

use super::{
    model::TaskManager,
    pager_view::tail_rows,
    view::stream_name,
    view_list::{confirm_rows, detail_rows, filters, heading, task_row, DETAIL_ROWS},
};
use crate::i18n::text as t;
use crate::render::panel_palette::{fg, PanelPalette};

/// 筛选行、分隔线、详情标题与详情字段占用的固定行数
const FIXED_ROWS: usize = 3 + DETAIL_ROWS;
/// 输出区至少需要标题加一行日志，不足时整块省略
const MIN_OUTPUT_ROWS: usize = 2;

/// 列表模式的行数分配，任务视口在前、尾部输出在后
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct ListLayout {
    pub tasks: usize,
    pub output: usize,
}

impl ListLayout {
    /// 【后台任务】【高度分配】任务视口按任务总数取高，至多占可变行的三分之二，其余留给尾部输出。
    /// 参数: rows 为正文行数，total 为任务总数（与筛选无关，切换范围时布局不跳动）；返回: 行数分配。
    pub fn compute(rows: usize, total: usize) -> Self {
        let flexible = rows.saturating_sub(FIXED_ROWS);
        let cap = (flexible * 2 / 3).max(flexible.min(3)).max(1);
        let tasks = total.clamp(1, cap);
        let output = flexible.saturating_sub(tasks);
        if output < MIN_OUTPUT_ROWS {
            Self {
                tasks: flexible.max(1),
                output: 0,
            }
        } else {
            Self { tasks, output }
        }
    }
}

/// 【后台任务】【列表正文】筛选、任务视口、详情和尾部输出依次排列，补齐到正文高度。
/// 参数: manager 为状态，inner 为内容宽度，rows 为正文行数，top 为正文首行的终端行号，targets 收集点击区域，palette 为主题；返回: 正文行与任务视口行数。
pub(super) fn rows(
    manager: &TaskManager,
    inner: usize,
    rows: usize,
    top: u16,
    targets: &mut Vec<(u16, String)>,
    palette: &PanelPalette,
) -> (Vec<String>, usize) {
    let layout = ListLayout::compute(rows, manager.tasks.len());
    let running = manager
        .tasks
        .iter()
        .filter(|task| task.status == "running")
        .count();
    let mut body = vec![
        filters(manager, running, palette),
        format!("{}{}\x1b[0m", fg(palette.border), "─".repeat(inner)),
    ];
    // 1. 【后台任务】【任务视口】选中行始终可见，视口高度固定，点击区域按终端行号记录
    let visible = manager.visible();
    let selected = visible
        .iter()
        .position(|index| Some(&manager.tasks[*index].id) == manager.selected_id.as_ref())
        .unwrap_or(0);
    let start = selected
        .saturating_sub(layout.tasks - 1)
        .min(visible.len().saturating_sub(layout.tasks));
    for index in visible.iter().skip(start).take(layout.tasks) {
        let task = &manager.tasks[*index];
        targets.push((top + body.len() as u16, task.id.clone()));
        body.push(task_row(
            task,
            Some(&task.id) == manager.selected_id.as_ref(),
            manager.confirm.is_none() && Some(&task.id) == manager.hovered.as_ref(),
            inner,
            palette,
        ));
    }
    if visible.is_empty() {
        body.push(format!(
            " {}{}\x1b[0m",
            fg(palette.muted),
            if manager.loading {
                t("Loading tasks...", "正在获取任务…")
            } else {
                t("No matching tasks", "暂无匹配任务")
            }
        ));
    }
    body.resize(layout.tasks + 2, String::new());
    // 2. 【后台任务】【下半区】确认时换成目标信息且列表不响应点击，否则显示详情与尾部输出
    if let Some(request) = &manager.confirm {
        targets.clear();
        body.extend(confirm_rows(request, manager, inner, palette));
    } else {
        let label = format!("\x1b[1m{}{}", fg(palette.muted), t("Details", "选中详情"));
        body.push(heading(&label, inner, palette));
        body.extend(detail_rows(manager, palette));
        if layout.output > 0 && manager.selected().is_some() {
            let logs = manager.log_lines();
            body.push(heading(
                &output_label(manager, logs.len(), palette),
                inner,
                palette,
            ));
            body.extend(tail_rows(manager, &logs, inner, layout.output - 1, palette));
        }
    }
    body.resize(rows, String::new());
    (body, layout.tasks)
}

/// 【后台任务】【输出标题】标题加粗减淡，附当前通道和已读取的行数。
/// 参数: manager 为状态，lines 为当前通道行数，palette 为主题；返回: 已着色标题。
fn output_label(manager: &TaskManager, lines: usize, palette: &PanelPalette) -> String {
    let loaded = manager.output_id == manager.selected_id && manager.output_error.is_none();
    let count = if loaded {
        format!(" · {lines} {}", t("lines", "行"))
    } else {
        String::new()
    };
    format!(
        "\x1b[1m{}{}\x1b[22m · {}{count}",
        fg(palette.muted),
        t("Output", "输出"),
        stream_name(manager.stream),
    )
}
