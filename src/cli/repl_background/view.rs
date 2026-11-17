//! 圆角任务面板：标题胶囊、响应式双栏/堆叠正文或全屏日志，尺寸由调用方给出的矩形决定。

use super::{
    list_body,
    model::{safe_text, TaskManager},
    panel_rect::PanelRect,
    theme::{fit, pill, stream_name, RESET},
    view_hints::footer_with_hits,
};
use crate::i18n::text as t;
use crate::render::ansi_clip::ansi_visible_width;
use crate::render::panel_palette::{fg, PanelPalette};
use crossterm::event::KeyCode;
use std::ops::Range;

/// 【后台任务】【按钮几何】记录渲染出来的热区所在终端坐标与触发按键。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct ControlHit {
    pub row: u16,
    pub start_col: u16,
    pub end_col: u16,
    pub key: KeyCode,
}

pub(super) struct TaskFrame {
    /// 面板自身的行，每行恰好占 rect.width 列，不含左侧偏移
    pub lines: Vec<String>,
    /// 任务行的终端行号与任务标识
    pub targets: Vec<(u16, String)>,
    /// 已渲染控件的精确可点击热区（筛选标签、通道胶囊、命令展开按钮、页脚按钮）
    pub buttons: Vec<ControlHit>,
    /// 详情区展开命令时的独立滚轮矩形及命令分页行数
    pub cmd_rect: Option<PanelRect>,
    pub cmd_visible_rows: usize,
    pub cmd_total_rows: usize,
    /// 尾部输出区的终端屏幕范围与可见行数（用于独立滚轮滚动）
    pub tail_rect: Option<PanelRect>,
    pub tail_rows: usize,
    pub page_rows: usize,
    pub rect: PanelRect,
}

impl TaskFrame {
    /// 【后台任务】【整屏定位】独立管理屏幕没有底层内容，按矩形位置补齐上方空行与左侧空格。
    ///
    /// 参数: 无
    ///
    /// 返回:
    /// - 从终端首行开始的整屏行
    pub fn placed(&self) -> Vec<String> {
        let indent = " ".repeat(usize::from(self.rect.x));
        let mut lines = vec![String::new(); usize::from(self.rect.y)];
        lines.extend(self.lines.iter().map(|line| format!("{indent}{line}")));
        lines
    }
}

/// 【后台任务】【面板组装】按给定矩形绘制圆角工作台，自动适应宽屏双栏与窄屏堆叠。
///
/// 参数:
/// - `manager`: 任务管理状态
/// - `rect`: 面板外框所在矩形
///
/// 返回:
/// - 包含视觉行与精确交互热区的帧结果
pub(super) fn render(manager: &mut TaskManager, rect: PanelRect) -> TaskFrame {
    let palette = PanelPalette::current();
    let inner = usize::from(rect.width.saturating_sub(4));
    let body_rows = usize::from(rect.height.saturating_sub(2));
    let bottom = rect.y.saturating_add(rect.height.saturating_sub(1));
    let mut targets = Vec::new();
    let mut buttons = Vec::new();
    let mut thumb = 0..0;
    let mut cmd_rect = None;
    let mut cmd_visible_rows = 0usize;
    let mut cmd_total_rows = 0usize;
    let mut tail_rect = None;
    let mut tail_rows = 0usize;
    let content_left = usize::from(rect.x) + 2;
    let (title, body, page_rows) = if manager.pager {
        // 1. 【后台任务】【日志模式】首行为状态顶栏，其余正文行全部用于日志输出
        let page_rows = body_rows.saturating_sub(1).max(1);
        let (body, rows, header_hits) = super::pager_view::rows(manager, inner, page_rows);
        thumb = rows;
        if body_rows > 0 && rect.width >= 4 {
            let row = rect.y.saturating_add(1);
            if row < bottom {
                for item in header_hits {
                    buttons.push(ControlHit {
                        row,
                        start_col: (content_left + item.start) as u16,
                        end_col: (content_left + item.end) as u16,
                        key: item.key,
                    });
                }
            }
        }
        (pager_title(manager, &palette), body, page_rows)
    } else {
        let top = rect.y.saturating_add(1);
        let rendered = list_body::render(
            manager,
            inner,
            body_rows,
            content_left as u16,
            top,
            &mut targets,
            &palette,
        );
        cmd_rect = rendered.cmd_rect;
        cmd_visible_rows = rendered.cmd_visible_rows;
        cmd_total_rows = rendered.cmd_total_rows;
        tail_rect = rendered.tail_rect;
        tail_rows = rendered.tail_rows;
        if body_rows > 0 && rect.width >= 4 {
            if top < bottom {
                for item in rendered.ribbon_hits {
                    buttons.push(ControlHit {
                        row: top,
                        start_col: (content_left + item.start) as u16,
                        end_col: (content_left + item.end) as u16,
                        key: item.key,
                    });
                }
            }
            buttons.extend(rendered.detail_hits.into_iter().filter(|hit| hit.row < bottom));
        }
        (
            list_title(manager, &palette),
            rendered.lines,
            rendered.task_viewport_rows,
        )
    };
    let (footer_text, footer_hits) = footer_with_hits(manager, inner);
    if rect.height >= 2 && rect.width >= 4 {
        for item in footer_hits {
            buttons.push(ControlHit {
                row: bottom,
                start_col: (content_left + item.start) as u16,
                end_col: (content_left + item.end) as u16,
                key: item.key,
            });
        }
    }
    // 2. 【后台任务】【外框组装】右边框兼作日志滚动条，滑块所在行显示高亮粗线
    let mut lines = vec![border('╭', '╮', &title, rect.width, &palette)];
    lines.extend(
        body.into_iter()
            .take(body_rows)
            .enumerate()
            .map(|(index, line)| boxed(&line, rect.width, thumb.contains(&index), &palette)),
    );
    if rect.height >= 2 {
        lines.push(border('╰', '╯', &footer_text, rect.width, &palette));
    }
    lines.truncate(usize::from(rect.height));
    // 3. 【后台任务】【命中裁剪】极小高度下被底框挤掉的任务与按钮热区不再响应点击
    targets.retain(|(row, _)| *row < bottom);
    buttons.retain(|hit| hit.row <= bottom);
    // 4. 【后台任务】【按钮悬停】使用保色相悬停高亮突出指针所在的可点击胶囊与页脚按键
    if let Some((column, row)) = manager.hovered_button {
        if let Some(button) = buttons
            .iter()
            .find(|hit| hit.row == row && column >= hit.start_col && column < hit.end_col)
        {
            let index = usize::from(button.row.saturating_sub(rect.y));
            if let Some(line) = lines.get_mut(index) {
                let from = usize::from(button.start_col.saturating_sub(rect.x));
                let to = usize::from(button.end_col.saturating_sub(rect.x));
                *line = crate::cli::repl_runtime::control_tint_cols(line, from, to);
            }
        }
    }
    TaskFrame {
        lines,
        targets,
        buttons,
        cmd_rect,
        cmd_visible_rows,
        cmd_total_rows,
        tail_rect,
        tail_rows,
        page_rows,
        rect,
    }
}

/// 【后台任务】【列表标题】主题强调符号、范围名称与运行/总计胶囊。
///
/// 参数:
/// - `manager`: 任务管理状态
/// - `palette`: 当前面板色板
///
/// 返回:
/// - 嵌入顶框的标题文本
fn list_title(manager: &TaskManager, palette: &PanelPalette) -> String {
    let running = manager
        .tasks
        .iter()
        .filter(|task| task.status == "running")
        .count();
    let badge_color = if running > 0 {
        palette.accent
    } else {
        palette.muted
    };
    let count_pill = pill(
        &format!(
            " {running}/{} {} ",
            manager.tasks.len(),
            t("running", "运行中")
        ),
        badge_color,
        Some(palette.focus_bg),
        false,
    );
    format!(
        "{}◆\x1b[39m \x1b[1m{}\x1b[22m {count_pill}{RESET}",
        fg(palette.accent),
        if manager.session_scoped {
            t("Background tasks · Current session", "后台任务 · 当前会话")
        } else {
            t("Background tasks · All sessions", "后台任务 · 全部会话")
        },
    )
}

/// 【后台任务】【日志标题】任务标识加粗，输出通道使用胶囊，跟随状态着色区分。
///
/// 参数:
/// - `manager`: 任务管理状态
/// - `palette`: 当前面板色板
///
/// 返回:
/// - 嵌入顶框的日志模式标题
fn pager_title(manager: &TaskManager, palette: &PanelPalette) -> String {
    let id = manager.selected_id.as_deref().unwrap_or_default();
    let follow = if manager.follow {
        t("Follow on", "跟随开启")
    } else {
        t("Follow off", "跟随暂停")
    };
    let follow_color = if manager.follow {
        palette.added
    } else {
        palette.muted
    };
    format!(
        "{}◆\x1b[39m \x1b[1m{}: {}\x1b[22m {}· {}{} {}{}{RESET}",
        fg(palette.accent),
        t("Task logs", "任务日志"),
        safe_text(id),
        fg(palette.muted),
        fg(palette.accent),
        stream_name(manager.stream),
        fg(palette.muted),
        format!("· {}{follow}", fg(follow_color)),
    )
}

/// 【后台任务】【相对时长】将秒数格式化为紧凑易读的时间字符串。
///
/// 参数:
/// - `seconds`: 经过秒数
///
/// 返回:
/// - 时长字符串
pub(super) fn duration(seconds: u64) -> String {
    if seconds >= 3600 {
        format!("{}h {}m", seconds / 3600, seconds % 3600 / 60)
    } else if seconds >= 60 {
        format!("{}m {}s", seconds / 60, seconds % 60)
    } else {
        format!("{seconds}s")
    }
}

/// 【后台任务】【日志滑块】按视口与缓冲区比例计算滑块所在的日志行，未溢出时不显示。
///
/// 参数:
/// - `offset`: 首行偏移
/// - `visible`: 视口行数
/// - `total`: 日志总行数
///
/// 返回:
/// - 相对日志首行的滑块范围
pub(super) fn thumb(offset: usize, visible: usize, total: usize) -> Range<usize> {
    if total <= visible || visible == 0 {
        return 0..0;
    }
    let size = (visible * visible / total).clamp(1, visible);
    let travel = visible - size;
    let maximum = total - visible;
    let top = offset.min(maximum) * travel / maximum;
    top..top + size
}

/// 【后台任务】【边框绘制】生成带嵌入信息的圆角边框，外框使用柔和边框色。
///
/// 参数:
/// - `left`: 左角字符
/// - `right`: 右角字符
/// - `label`: 嵌入边框的带样式文本
/// - `width`: 外框总宽
/// - `palette`: 当前面板色板
///
/// 返回:
/// - 恰好占 `width` 列的圆角边框行
fn border(left: char, right: char, label: &str, width: u16, palette: &PanelPalette) -> String {
    if width < 2 {
        return fit(&left.to_string(), usize::from(width));
    }
    let frame = fg(palette.border);
    let label = if label.is_empty() {
        String::new()
    } else {
        fit(&format!(" {label}\x1b[0m "), usize::from(width - 2))
    };
    format!(
        "{frame}{left}\x1b[0m{label}{frame}{}{right}\x1b[0m",
        "─".repeat(usize::from(width - 2).saturating_sub(ansi_visible_width(&label)))
    )
}

/// 【后台任务】【内容边框】补齐内容行并保持外框宽度，滑块所在行的右边框使用主题强调粗线。
///
/// 参数:
/// - `text`: 内容视觉行
/// - `width`: 外框总宽
/// - `thumb`: 是否处于日志滚动滑块范围内
/// - `palette`: 当前面板色板
///
/// 返回:
/// - 带左右边框的内容行
fn boxed(text: &str, width: u16, thumb: bool, palette: &PanelPalette) -> String {
    if width < 4 {
        return fit("││", usize::from(width));
    }
    let frame = fg(palette.border);
    let inner = usize::from(width - 4);
    let text = fit(text, inner);
    let (right, color) = if thumb {
        ('┃', fg(palette.accent))
    } else {
        ('│', frame.clone())
    };
    format!(
        "{frame}│\x1b[0m {text}\x1b[0m{} {color}{right}\x1b[0m",
        " ".repeat(inner.saturating_sub(ansi_visible_width(&text)))
    )
}
