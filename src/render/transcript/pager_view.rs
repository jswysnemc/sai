//! 副屏正文：和前台同一套 cell 渲染。
//!
//! 当前段落强制展开，其余可折叠段落强制保持前台的折叠样子。
//! 助手正文、短回显这些本来就不折叠的内容，两边画出来一样。

use super::line::AnsiLine;
use super::spacing;
use super::store::{TranscriptRenderOptions, TranscriptStore, TranscriptView};
use crate::llm::ChatStreamKind;
use crate::render::content_indent::{align_to_guide_column_with_width, CONTENT_LEFT_INDENT};
use crate::render::render_expand::{with_force_collapse, with_force_expand};

/// 副屏一帧：屏幕上的行，以及用来搜索的全文。
pub(crate) struct PagerView {
    /// 当前焦点下的屏幕行（已加左侧留白）
    pub(crate) display: Vec<AnsiLine>,
    /// 屏幕行对应的搜索行；折叠段没有可高亮的正文位置
    pub(crate) source: Vec<Option<usize>>,
    /// 每个可切换段落在屏幕行中的起点
    pub(crate) starts: Vec<usize>,
    /// 全部段落按展开后的正文拼成的搜索文档
    pub(crate) search_lines: Vec<AnsiLine>,
    /// 搜索行属于哪一段；普通正文没有段落编号
    pub(crate) search_paragraph: Vec<Option<usize>>,
    pub(crate) paragraphs: usize,
}

impl PagerView {
    /// 还没画出正文时的空帧。
    pub(crate) fn empty() -> Self {
        Self {
            display: Vec::new(),
            source: Vec::new(),
            starts: Vec::new(),
            search_lines: Vec::new(),
            search_paragraph: Vec::new(),
            paragraphs: 0,
        }
    }

    /// 单段纯文本，给不经过 transcript 的文本分页用。
    pub(crate) fn from_lines(lines: Vec<AnsiLine>) -> Self {
        let search_paragraph = vec![Some(0); lines.len()];
        let source = (0..lines.len()).map(Some).collect();
        Self {
            starts: vec![0],
            search_lines: lines.clone(),
            search_paragraph,
            paragraphs: 1,
            display: lines,
            source,
        }
    }
}

/// 一段已经折好的正文。可切换段落同时保留折叠和展开两份。
struct Segment {
    paragraph: Option<usize>,
    expanded: Vec<AnsiLine>,
    collapsed: Vec<AnsiLine>,
}

impl TranscriptStore {
    /// 按前台的宽度和留白渲染副屏。`focus` 是要展开的段落。
    ///
    /// 参数: `width` 为副屏正文列数，`options` 为前台渲染选项，`focus` 为当前段落
    /// 返回: 屏幕行和搜索文档
    pub(crate) fn render_pager_view(
        &mut self,
        width: usize,
        options: &TranscriptRenderOptions,
        focus: usize,
    ) -> PagerView {
        let width = width.max(1);
        if let TranscriptView::Subagent { id, label } = self.view.clone() {
            let lines = super::subagent_view::render_view_lines(
                &id,
                &label,
                width,
                self.live_animation_frame(),
            );
            return PagerView::from_lines(guide_lines(lines, width));
        }
        let padding = CONTENT_LEFT_INDENT.min(width.saturating_sub(1));
        let content_width = width.saturating_sub(padding).max(1);
        let frame = self.live_animation_frame();
        let mut paragraph = 0usize;
        let mut segments = Vec::new();
        for (index, cell) in self.cells.iter().enumerate() {
            let expandable = TranscriptStore::is_expandable_cell(cell);
            let id = expandable.then(|| {
                let id = paragraph;
                paragraph += 1;
                id
            });
            let gap = index > 0 && spacing::needs_section_gap(&self.cells[index - 1], cell);
            let collapsed = if expandable {
                with_force_collapse(|| cell.display_lines_framed(content_width, options, frame))
            } else {
                cell.display_lines_framed(content_width, options, frame)
            };
            let expanded = if expandable {
                with_force_expand(|| cell.display_lines_framed(content_width, options, frame))
            } else {
                collapsed.clone()
            };
            segments.push(segment_with_gap(id, gap, expanded, collapsed));
        }
        let live_paragraph = self.live_tail.as_ref().is_some_and(|tail| {
            tail.kind == ChatStreamKind::Reasoning && !tail.source.trim().is_empty()
        });
        if live_paragraph {
            let id = paragraph;
            paragraph += 1;
            let collapsed =
                with_force_collapse(|| self.render_live_tail(content_width, options, usize::MAX));
            let expanded =
                with_force_expand(|| self.render_live_tail(content_width, options, usize::MAX));
            segments.push(Segment {
                paragraph: Some(id),
                expanded,
                collapsed,
            });
        } else {
            let lines = self.render_live_tail(content_width, options, usize::MAX);
            if !lines.is_empty() {
                segments.push(Segment {
                    paragraph: None,
                    expanded: lines.clone(),
                    collapsed: lines,
                });
            }
        }
        let focus = focus.min(paragraph.saturating_sub(1));
        assemble(segments, focus, paragraph, width)
    }

    /// 副屏有没有东西可看。
    pub(crate) fn has_pager_content(&self) -> bool {
        !self.cells.is_empty()
            || self
                .live_tail
                .as_ref()
                .is_some_and(|tail| !tail.source.is_empty())
            || self.live_tool_call.is_some()
            || self.work_status.is_some()
    }
}

fn segment_with_gap(
    paragraph: Option<usize>,
    gap: bool,
    mut expanded: Vec<AnsiLine>,
    mut collapsed: Vec<AnsiLine>,
) -> Segment {
    if gap {
        expanded.insert(0, AnsiLine::new(String::new()));
        collapsed.insert(0, AnsiLine::new(String::new()));
    }
    Segment {
        paragraph,
        expanded,
        collapsed,
    }
}

fn assemble(segments: Vec<Segment>, focus: usize, paragraphs: usize, width: usize) -> PagerView {
    let mut display = Vec::new();
    let mut source = Vec::new();
    let mut starts = vec![0; paragraphs];
    let mut search_lines = Vec::new();
    let mut search_paragraph = Vec::new();
    for segment in segments {
        let search_base = search_lines.len();
        for line in &segment.expanded {
            search_lines.push(guide_line(line, width));
            search_paragraph.push(segment.paragraph);
        }
        let shown = match segment.paragraph {
            Some(id) if id == focus => &segment.expanded,
            Some(_) => &segment.collapsed,
            None => &segment.expanded,
        };
        if let Some(id) = segment.paragraph {
            if id < starts.len() {
                starts[id] = display.len();
            }
        }
        let focused = segment.paragraph == Some(focus);
        let mapped = segment.paragraph.is_none() || focused;
        for (offset, line) in shown.iter().enumerate() {
            display.push(guide_line(line, width));
            source.push(mapped.then_some(search_base + offset));
        }
    }
    PagerView {
        display,
        source,
        starts,
        search_lines,
        search_paragraph,
        paragraphs,
    }
}

fn guide_lines(lines: Vec<AnsiLine>, width: usize) -> Vec<AnsiLine> {
    lines
        .into_iter()
        .map(|line| guide_line(&line, width))
        .collect()
}

fn guide_line(line: &AnsiLine, width: usize) -> AnsiLine {
    let padding = CONTENT_LEFT_INDENT.min(width.saturating_sub(1));
    AnsiLine::new(align_to_guide_column_with_width(line.as_str(), padding))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::activity_animation::strip_ansi_for_test;
    use crate::render::{ReasoningDisplayMode, ToolCallDisplayMode};

    fn options() -> TranscriptRenderOptions {
        TranscriptRenderOptions {
            reasoning_mode: ReasoningDisplayMode::Full,
            tool_call_mode: ToolCallDisplayMode::Full,
        }
    }

    fn plain(view: &PagerView) -> String {
        view.display
            .iter()
            .map(|line| strip_ansi_for_test(line.as_str()))
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    fn focused_paragraph_uses_foreground_expansion() {
        let mut store = TranscriptStore::new(80);
        store.push_chunk(&crate::llm::ChatStreamChunk {
            kind: ChatStreamKind::Reasoning,
            text: (0..12).map(|i| format!("thinking-line-{i}\n")).collect(),
        });
        store.finalize_live_tail();
        store.push_chunk(&crate::llm::ChatStreamChunk {
            kind: ChatStreamKind::Content,
            text: "assistant stays visible".into(),
        });
        store.finalize_live_tail();
        store.push_chunk(&crate::llm::ChatStreamChunk {
            kind: ChatStreamKind::Reasoning,
            text: (0..12).map(|i| format!("later-line-{i}\n")).collect(),
        });
        store.finalize_live_tail();

        let focused = store.render_pager_view(80, &options(), 0);
        let text = plain(&focused);
        assert!(text.contains("thinking-line-6"), "{text}");
        assert!(text.contains("assistant stays visible"), "{text}");
        assert!(!text.contains("later-line-6"), "{text}");
        assert!(text.contains("Ctrl+O") || text.contains('▸'), "{text}");

        let other = store.render_pager_view(80, &options(), 1);
        let text = plain(&other);
        assert!(text.contains("later-line-6"), "{text}");
        assert!(!text.contains("thinking-line-6"), "{text}");
        assert!(text.contains("assistant stays visible"), "{text}");
    }
}
