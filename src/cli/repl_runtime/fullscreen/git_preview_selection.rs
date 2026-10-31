//! 差异正文的源文本选区，不把行号、增删标记或面板边框复制进去。

use super::git_preview::GitPreview;
use super::git_preview_patch::{LineKind, PatchLine};
use super::git_preview_view::PreviewLayout;
use super::input::Effect;
use super::selection::{self, Selection, TextPoint};
use crate::cli::repl_runtime::ReplRuntime;
use crossterm::event::{Event, MouseButton, MouseEvent, MouseEventKind};

#[derive(Default)]
pub(super) struct DiffSelection {
    press: Option<TextPoint>,
    pub(super) span: Option<Selection>,
}

impl DiffSelection {
    /// 【终端审查】【鼠标归属】判断差异是否持有按下状态，顶部控件不能抢走释放事件。
    /// 参数: 无；返回: 是否正在拖选。
    pub(super) fn is_pressed(&self) -> bool {
        self.press.is_some()
    }
}

/// 【终端审查】【代码边界】返回正文相对差异行起点的列数，与绘制行号宽度保持一致。
/// 参数: line 为差异行；返回: 固定排版栏宽度。
fn gutter(line: &PatchLine) -> usize {
    match line.kind {
        LineKind::Hunk => 2,
        LineKind::Notice => 0,
        _ => super::git_preview_code::gutter_width(line),
    }
}

/// 【终端审查】【源偏移】块标题和通知不横向滚动，代码行使用当前代码偏移。
/// 参数: line 为差异行，horizontal 为横向偏移；返回: 实际源列偏移。
fn horizontal(line: &PatchLine, horizontal: usize) -> usize {
    if matches!(line.kind, LineKind::Hunk | LineKind::Notice) {
        0
    } else {
        horizontal
    }
}

/// 【终端审查】【源坐标】按纵横滚动偏移映射鼠标，拖出面板时夹紧到代码边界。
/// 参数: preview 为差异，geometry 为面板，mouse 为指针；返回: 有效源文本坐标。
fn point(preview: &GitPreview, geometry: &PreviewLayout, mouse: &MouseEvent) -> Option<TextPoint> {
    let lines = &preview.content.as_ref()?.lines;
    let row = preview.scroll
        + usize::from(mouse.row)
            .saturating_sub(geometry.diff_top)
            .min(geometry.visible.saturating_sub(1));
    let row = row.min(lines.len().checked_sub(1)?);
    let line = &lines[row];
    let start = geometry.left + 2 + gutter(line);
    let visible = geometry.width.saturating_sub(3 + gutter(line));
    let col = horizontal(line, preview.horizontal)
        + usize::from(mouse.column).saturating_sub(start).min(visible);
    Some(TextPoint {
        row,
        col: col.min(crate::cli::repl_text::visible_width(&line.text)),
    })
}

/// 【终端审查】【复制正文】只扫描覆盖的源代码行，保留内容缩进并去除行尾填充。
/// 参数: preview 为差异状态；返回: 当前选区文本，无选区时为空。
pub(super) fn selected_text(preview: &GitPreview) -> String {
    let (Some(span), Some(content)) = (preview.selection.span, preview.content.as_ref()) else {
        return String::new();
    };
    let (start, end) = span.ordered();
    let Some(lines) = content
        .lines
        .get(start.row..end.row.saturating_add(1).min(content.lines.len()))
    else {
        return String::new();
    };
    lines
        .iter()
        .enumerate()
        .filter_map(|(offset, line)| {
            span.cols_on_row(start.row + offset, usize::MAX)
                .map(|(from, to)| {
                    selection::slice_cols(&line.text, from, to)
                        .trim_end()
                        .to_string()
                })
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// 【终端审查】【选区着色】仅高亮可见代码，原有语法色和差异背景保持闭合。
/// 参数: preview 为状态，index 为源行，text 为定宽渲染行，width 为净宽；返回: 高亮行。
pub(super) fn highlight(preview: &GitPreview, index: usize, text: String, width: usize) -> String {
    let Some(span) = preview.selection.span else {
        return text;
    };
    let Some(line) = preview
        .content
        .as_ref()
        .and_then(|content| content.lines.get(index))
    else {
        return text;
    };
    let Some((from, to)) =
        span.cols_on_row(index, crate::cli::repl_text::visible_width(&line.text))
    else {
        return text;
    };
    let gutter = gutter(line);
    let horizontal = horizontal(line, preview.horizontal);
    let from = gutter + from.saturating_sub(horizontal);
    let to = (gutter + to.saturating_sub(horizontal)).min(width);
    selection::highlight_cols(&text, from, to)
}

impl ReplRuntime {
    /// 【终端审查】【拖选捕获】按下正文后捕获拖动和释放，离开右栏也不触发聊天或尺寸调整。
    /// 参数: event 为终端事件；返回: 处理效果，非选区事件为空。
    pub(super) fn git_preview_selection_event(&mut self, event: &Event) -> Option<Effect> {
        let session = self.fullscreen.as_mut()?;
        let geometry = PreviewLayout::compute(
            &session.state.layout?,
            session.sidebar.git.summary.files.len(),
        );
        let preview = &mut session.sidebar.preview;
        preview.path.as_ref()?;
        let Event::Mouse(mouse) = event else {
            if matches!(event, Event::FocusLost | Event::Resize(..)) {
                preview.selection.press = None;
            }
            return None;
        };
        if matches!(mouse.kind, MouseEventKind::Down(MouseButton::Left)) {
            preview.selection.span = None;
            preview.selection.press = None;
            if geometry.contains(mouse.column, mouse.row)
                && geometry.in_diff(mouse.row)
                && mouse.column > geometry.left as u16
            {
                let anchor = point(preview, &geometry, mouse)?;
                preview.selection.press = Some(anchor);
                preview.focus = Some(super::git_preview::ReviewFocus::Diff);
                preview.hovered = None;
                session.state.press = None;
                session.state.selection = None;
                session.state.copied = None;
                session.state.dragging = false;
                self.composer_selection.clear();
                return Some(Effect::Repaint);
            }
            // 1. 【终端审查】【选区失效】其他区域继续处理点击，由统一重绘清除旧高亮
            return None;
        }
        let anchor = preview.selection.press?;
        if !matches!(
            mouse.kind,
            MouseEventKind::Drag(MouseButton::Left) | MouseEventKind::Up(MouseButton::Left)
        ) {
            return None;
        }
        // 2. 【终端审查】【边缘滚动】只移动差异正文，固定文件名和左侧聊天不变
        if matches!(mouse.kind, MouseEventKind::Drag(_)) {
            if usize::from(mouse.row) < geometry.diff_top {
                preview.scroll_by(-1, geometry.visible);
            } else if usize::from(mouse.row) >= geometry.diff_top + geometry.visible {
                preview.scroll_by(1, geometry.visible);
            }
        }
        let head = point(preview, &geometry, mouse)?;
        preview.selection.span = Some(Selection { anchor, head });
        if matches!(mouse.kind, MouseEventKind::Up(_)) {
            preview.selection.press = None;
            let text = selected_text(preview);
            if !text.is_empty() {
                session.state.copied = Some(text.chars().count());
                session.pending_copy = Some(text);
            }
        }
        Some(Effect::Repaint)
    }
}
