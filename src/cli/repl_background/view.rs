//! 圆角任务面板：标题、列表或日志正文与底框提示的帧组装，尺寸由调用方给出的矩形决定。

use super::{
    list_body,
    model::{safe_text, Stream, TaskManager},
    panel_rect::PanelRect,
    view_hints::footer,
};
use crate::i18n::text as t;
use crate::render::ansi_clip::{ansi_visible_width, clip_ansi_to_width};
use crate::render::panel_palette::{fg, PanelPalette};
use std::ops::Range;

pub(super) struct TaskFrame {
    /// 面板自身的行，每行恰好占 rect.width 列，不含左侧偏移
    pub lines: Vec<String>,
    /// 任务行的终端行号与任务标识
    pub targets: Vec<(u16, String)>,
    pub page_rows: usize,
    pub rect: PanelRect,
}

impl TaskFrame {
    /// 【后台任务】【整屏定位】独立管理屏幕没有底层内容，按矩形位置补齐上方空行与左侧空格。
    /// 参数: 无；返回: 从终端首行开始的整屏行。
    pub fn placed(&self) -> Vec<String> {
        let indent = " ".repeat(usize::from(self.rect.x));
        let mut lines = vec![String::new(); usize::from(self.rect.y)];
        lines.extend(self.lines.iter().map(|line| format!("{indent}{line}")));
        lines
    }
}

/// 【后台任务】【面板组装】按给定矩形绘制面板，列表模式在任务与尾部输出之间分配高度。
/// 参数: manager 为状态，rect 为面板外框所在矩形；返回: 面板行与点击区域。
pub(super) fn render(manager: &mut TaskManager, rect: PanelRect) -> TaskFrame {
    let palette = PanelPalette::current();
    let inner = usize::from(rect.width.saturating_sub(4));
    let body_rows = usize::from(rect.height.saturating_sub(2));
    let mut targets = Vec::new();
    let mut thumb = 0..0;
    let (title, body, page_rows) = if manager.pager {
        // 1. 【后台任务】【日志模式】首行为缓冲区提示，其余正文行全部用于日志
        let page_rows = body_rows.saturating_sub(1).max(1);
        let (body, rows) = super::pager_view::rows(manager, inner, page_rows);
        thumb = rows;
        (pager_title(manager, &palette), body, page_rows)
    } else {
        let top = rect.y.saturating_add(1);
        let (body, tasks) = list_body::rows(manager, inner, body_rows, top, &mut targets, &palette);
        (list_title(manager, &palette), body, tasks)
    };
    let footer = footer(manager, inner);
    // 2. 【后台任务】【外框组装】右边框兼作日志滚动位置，滑块所在行使用粗线
    let mut lines = vec![border('╭', '╮', &title, rect.width, &palette)];
    lines.extend(
        body.into_iter()
            .take(body_rows)
            .enumerate()
            .map(|(index, line)| boxed(&line, rect.width, thumb.contains(&index), &palette)),
    );
    if rect.height >= 2 {
        lines.push(border('╰', '╯', &footer, rect.width, &palette));
    }
    lines.truncate(usize::from(rect.height));
    // 3. 【后台任务】【命中裁剪】极小高度下被底框挤掉的任务行不再响应点击
    let bottom = rect.y.saturating_add(rect.height.saturating_sub(1));
    targets.retain(|(row, _)| *row < bottom);
    TaskFrame {
        lines,
        targets,
        page_rows,
        rect,
    }
}

/// 【后台任务】【列表标题】强调色标记、加粗名称与减淡的运行计数。
/// 参数: manager 为状态，palette 为主题；返回: 嵌入上边框的标题。
fn list_title(manager: &TaskManager, palette: &PanelPalette) -> String {
    let running = manager
        .tasks
        .iter()
        .filter(|task| task.status == "running")
        .count();
    format!(
        "{}◆\x1b[39m \x1b[1m{}\x1b[22m {}{running}/{} {}\x1b[0m",
        fg(palette.accent),
        if manager.session_scoped {
            t("Background tasks · Current session", "后台任务 · 当前会话")
        } else {
            t("Background tasks · All sessions", "后台任务 · 全部会话")
        },
        fg(palette.muted),
        manager.tasks.len(),
        t("running", "运行中")
    )
}

/// 【后台任务】【日志标题】任务标识加粗，输出通道使用强调色，跟随状态减淡。
/// 参数: manager 为状态，palette 为主题；返回: 嵌入上边框的标题。
fn pager_title(manager: &TaskManager, palette: &PanelPalette) -> String {
    let id = manager.selected_id.as_deref().unwrap_or_default();
    let follow = if manager.follow {
        t("Follow on", "跟随开启")
    } else {
        t("Follow off", "跟随暂停")
    };
    format!(
        "\x1b[1m{}: {}\x1b[22m {}· {}{}{} · {follow}\x1b[0m",
        t("Task logs", "任务日志"),
        safe_text(id),
        fg(palette.muted),
        fg(palette.accent),
        stream_name(manager.stream),
        fg(palette.muted),
    )
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

/// 【后台任务】【日志滑块】按视口与缓冲区比例计算滑块所在的日志行，未溢出时不显示。
/// 参数: offset 为首行偏移，visible 为视口行数，total 为日志总行数；返回: 相对日志首行的滑块范围。
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

/// 【后台任务】【边框绘制】生成带嵌入信息的圆角边框，外框使用主题弱化色。
/// 参数: left/right 为边角，label 为可信样式文本，width 为外框宽度，palette 为主题；返回: 边框行。
fn border(left: char, right: char, label: &str, width: u16, palette: &PanelPalette) -> String {
    if width < 2 {
        return fit(&left.to_string(), usize::from(width));
    }
    let frame = fg(palette.muted);
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

/// 【后台任务】【内容边框】补齐内容行并保持外框宽度，滑块所在行的右边框改为粗线。
/// 参数: text 为可信样式文本，width 为外框宽度，thumb 为是否滑块行，palette 为主题；返回: 带边框内容。
fn boxed(text: &str, width: u16, thumb: bool, palette: &PanelPalette) -> String {
    if width < 4 {
        return fit("││", usize::from(width));
    }
    let frame = fg(palette.muted);
    let inner = usize::from(width - 4);
    let text = fit(text, inner);
    let (right, color) = if thumb {
        ('┃', fg(palette.text))
    } else {
        ('│', frame.clone())
    };
    format!(
        "{frame}│\x1b[0m {text}\x1b[0m{} {color}{right}\x1b[0m",
        " ".repeat(inner.saturating_sub(ansi_visible_width(&text)))
    )
}
