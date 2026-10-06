//! 全屏会话视图的正文文档：按显式展开集合渲染完整 transcript。
//!
//! 与内联视图共用同一套 cell 渲染和区块空行规则；额外记录每个可折叠段落
//! 的行范围（供鼠标点击展开/收起）和每条用户消息的位置（供概览跳转与浮动标题）。

use super::cell::HistoryCell;
use super::fullscreen_parts::{is_segmented, render_segmented, ParagraphKey};
use super::line::AnsiLine;
use super::spacing;
use super::store::{TranscriptRenderOptions, TranscriptStore, TranscriptView};
use crate::llm::ChatStreamKind;
use crate::render::content_indent::{align_to_guide_column_with_width, CONTENT_LEFT_INDENT};
use crate::render::omitted_line::{
    rewrite_fold_hint_for_fullscreen, with_fullscreen_hints, EXPANDED_MARKER, FOLD_MARKER,
};
use crate::render::render_expand::{with_force_collapse, with_force_expand};
use std::collections::HashSet;

/// 流式思考尾部的段落键：定稿前没有稳定的 cell 下标。
pub(crate) const LIVE_PARAGRAPH_KEY: ParagraphKey = ParagraphKey {
    cell: usize::MAX,
    part: super::fullscreen_parts::ParagraphPart::Whole,
};
/// 概览摘要最多保留的字符数。
const SUMMARY_CHARS: usize = 120;

/// 一个可点击展开/收起的段落在文档中的位置。
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ParagraphSpan {
    /// 段落键：cell 下标与段落位置，流式思考尾部为 [`LIVE_PARAGRAPH_KEY`]
    pub(crate) key: ParagraphKey,
    /// 首行（段落标题行）在文档中的行号
    pub(crate) start: usize,
    /// 末行之后的行号
    pub(crate) end: usize,
    /// 当前是否展开
    pub(crate) expanded: bool,
    /// 可点击的控制行：折叠时为「N lines hidden」提示，展开时为末尾的「收起」行
    pub(crate) control: usize,
}

/// 一条用户消息在文档中的位置与摘要。
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct UserAnchor {
    /// 消息首行在文档中的行号
    pub(crate) row: usize,
    /// 单行摘要（已折叠空白）
    pub(crate) summary: String,
}

/// 全屏视图一帧的完整正文。
#[derive(Clone, Debug, Default)]
pub(crate) struct FullscreenDocument {
    pub(crate) lines: Vec<AnsiLine>,
    pub(crate) paragraphs: Vec<ParagraphSpan>,
    pub(crate) anchors: Vec<UserAnchor>,
}

impl FullscreenDocument {
    /// 查找包含指定行的可折叠段落。
    ///
    /// 参数:
    /// - `row`: 文档行号
    ///
    /// 返回:
    /// - 命中的段落
    #[cfg(test)]
    pub(crate) fn paragraph_at(&self, row: usize) -> Option<&ParagraphSpan> {
        self.paragraphs
            .iter()
            .find(|span| row >= span.start && row < span.end)
    }

    /// 查找控制行（折叠提示或收起行）恰好位于指定行的段落。
    ///
    /// 参数:
    /// - `row`: 文档行号
    ///
    /// 返回:
    /// - 命中的段落；正文其它行不响应悬停与点击
    pub(crate) fn paragraph_control_at(&self, row: usize) -> Option<&ParagraphSpan> {
        self.paragraphs.iter().find(|span| span.control == row)
    }

    /// 返回指定行所属的用户消息序号（该行之前最近的一条）。
    ///
    /// 参数:
    /// - `row`: 文档行号
    ///
    /// 返回:
    /// - 用户消息下标；首条消息之前为 None
    pub(crate) fn anchor_index_at(&self, row: usize) -> Option<usize> {
        self.anchors.iter().rposition(|anchor| anchor.row <= row)
    }
}

impl TranscriptStore {
    /// 【全屏视图】【正文文档】按宽度与展开集合渲染完整会话。
    ///
    /// 参数:
    /// - `width`: 正文区总列数，含左侧引导缩进；表格按扣除后的净宽排版
    /// - `options`: transcript 渲染选项
    /// - `expanded`: 需要展开的段落键
    ///
    /// 返回:
    /// - 完整正文、可折叠段落与用户消息位置
    pub(crate) fn render_fullscreen(
        &mut self,
        width: usize,
        options: &TranscriptRenderOptions,
        expanded: &HashSet<ParagraphKey>,
    ) -> FullscreenDocument {
        // 1. 【全屏视图】【正文净宽】先扣引导列再排 Markdown / 表格，右缘留给概览轨道
        let padding = CONTENT_LEFT_INDENT.min(width.saturating_sub(1));
        let content_width = width.saturating_sub(padding).max(1);
        let mut document = self.render_document(content_width, options, expanded);
        // 2. 全屏中 Ctrl+O 是退出键：折叠提示改为点击展开，再把引导列补回
        for line in &mut document.lines {
            let rewritten = rewrite_fold_hint_for_fullscreen(line.as_str());
            let source = rewritten.as_deref().unwrap_or(line.as_str());
            *line = AnsiLine::new(align_to_guide_column_with_width(source, padding));
        }
        document
    }

    /// 渲染全屏正文；调用方负责设置折叠提示上下文。
    ///
    /// 参数:
    /// - `width`: 正文列数
    /// - `options`: transcript 渲染选项
    /// - `expanded`: 需要展开的段落键
    ///
    /// 返回:
    /// - 完整正文、可折叠段落与用户消息位置
    fn render_document(
        &mut self,
        width: usize,
        options: &TranscriptRenderOptions,
        expanded: &HashSet<ParagraphKey>,
    ) -> FullscreenDocument {
        let width = width.max(1);
        let frame = self.live_animation_frame();
        // 1. 子智能体视图整体替换正文，没有可折叠段落和用户锚点
        if let TranscriptView::Subagent { id, label } = self.view.clone() {
            return FullscreenDocument {
                lines: super::subagent_view::render_view_lines(&id, &label, width, frame),
                ..FullscreenDocument::default()
            };
        }
        let mut document = FullscreenDocument::default();
        // 展开段一律按完整模式渲染：摘要模式下展开也要能看到正文
        let full = expanded_options();
        for index in 0..self.cells.len() {
            // 2. 区块空行与内联视图保持一致
            if index > 0 && spacing::needs_section_gap(&self.cells[index - 1], &self.cells[index]) {
                document.lines.push(AnsiLine::new(String::new()));
            }
            let start = document.lines.len();
            // 2. 命令卡片按命令行与输出分段，两段各自点击展开
            if is_segmented(&self.cells[index]) {
                let parts =
                    render_segmented(&self.cells[index], index, width, options, frame, |key| {
                        expanded.contains(&key)
                    });
                if let Some(parts) = parts {
                    push_segmented(&mut document, parts.lines, parts.spans);
                    continue;
                }
            }
            let key = ParagraphKey::whole(index);
            let expandable = TranscriptStore::is_expandable_cell(&self.cells[index]);
            let open = expandable && expanded.contains(&key);
            // 强制展开/折叠绕过缓存，可以直接在全屏提示上下文里渲染
            let lines = if open {
                let cell = &self.cells[index];
                with_fullscreen_hints(|| {
                    with_force_expand(|| cell.display_lines_framed(width, &full, frame))
                })
            } else if expandable {
                let cell = &self.cells[index];
                with_fullscreen_hints(|| {
                    with_force_collapse(|| cell.display_lines_framed(width, options, frame))
                })
            } else {
                self.cache
                    .lines_for(index, &self.cells[index], width, options, frame)
            };
            document.lines.extend(lines);
            // 3. 展开段末尾追加「收起」行，作为展开态唯一的点击入口
            if open {
                document.lines.push(AnsiLine::new(render_collapse_line()));
            }
            if let HistoryCell::UserEcho(cell) = &self.cells[index] {
                document.anchors.push(UserAnchor {
                    row: first_content_row(&document.lines, start),
                    summary: summarize(&cell.text),
                });
            }
            if expandable && document.lines.len() > start {
                document
                    .paragraphs
                    .push(make_span(&document.lines, key, start, document.lines.len(), open));
            }
        }
        self.append_live_tail(&mut document, width, options, expanded);
        document
    }

    /// 追加流式尾部；进行中的思考段同样可以点击展开。
    ///
    /// 参数:
    /// - `document`: 正在组装的文档
    /// - `width`: 正文列数
    /// - `options`: transcript 渲染选项
    /// - `expanded`: 需要展开的段落键
    ///
    /// 返回:
    /// - 无
    fn append_live_tail(
        &mut self,
        document: &mut FullscreenDocument,
        width: usize,
        options: &TranscriptRenderOptions,
        expanded: &HashSet<ParagraphKey>,
    ) {
        let reasoning = self.live_tail.as_ref().is_some_and(|tail| {
            tail.kind == ChatStreamKind::Reasoning && !tail.source.trim().is_empty()
        });
        let open = reasoning && expanded.contains(&LIVE_PARAGRAPH_KEY);
        let mut live = with_fullscreen_hints(|| {
            if open {
                let full = expanded_options();
                with_force_expand(|| self.render_live_tail(width, &full, usize::MAX))
            } else {
                self.render_live_tail(width, options, usize::MAX)
            }
        });
        // 定稿区末行已是空行时去掉 live 的前空行，与内联视图一致
        spacing::drop_duplicate_leading_blank(&mut live, document.lines.last());
        spacing::ensure_live_tool_gap(&mut live, self.cells.last());
        let start = document.lines.len();
        document.lines.extend(live);
        if open {
            document.lines.push(AnsiLine::new(render_collapse_line()));
        }
        if reasoning && document.lines.len() > start {
            document.paragraphs.push(make_span(
                &document.lines,
                LIVE_PARAGRAPH_KEY,
                start,
                document.lines.len(),
                open,
            ));
        }
    }
}

/// 展开段使用的渲染选项：思考与工具都按完整模式输出。
///
/// 返回:
/// - 完整模式渲染选项
fn expanded_options() -> TranscriptRenderOptions {
    TranscriptRenderOptions {
        reasoning_mode: crate::render::ReasoningDisplayMode::Full,
        tool_call_mode: crate::render::ToolCallDisplayMode::Full,
    }
}

/// 【全屏视图】【分段写入】把命令卡片的命令段与输出段写入文档，展开段各自追加收起行。
///
/// 参数:
/// - `document`: 正在组装的文档
/// - `lines`: 分段渲染后的行
/// - `spans`: 段落键、段内起止与展开状态
///
/// 返回:
/// - 无
fn push_segmented(
    document: &mut FullscreenDocument,
    lines: Vec<AnsiLine>,
    spans: Vec<(ParagraphKey, usize, usize, bool)>,
) {
    let mut copied = 0usize;
    for (key, from, to, open) in spans {
        // 1. 先补上两段之间未登记为段落的行
        document.lines.extend(lines[copied..from].iter().cloned());
        let start = document.lines.len();
        document.lines.extend(lines[from..to].iter().cloned());
        if open {
            document.lines.push(AnsiLine::new(render_collapse_line()));
        }
        let end = document.lines.len();
        document.paragraphs.push(make_span(&document.lines, key, start, end, open));
        copied = to;
    }
    document.lines.extend(lines[copied..].iter().cloned());
}

/// 构造段落范围并定位控制行。
///
/// 参数:
/// - `lines`: 文档行
/// - `key`: 段落键
/// - `start` / `end`: 段落行范围（含首部空行）
/// - `open`: 是否展开
///
/// 返回:
/// - 段落范围
fn make_span(
    lines: &[AnsiLine],
    key: ParagraphKey,
    start: usize,
    end: usize,
    open: bool,
) -> ParagraphSpan {
    let start = first_content_row(lines, start);
    let control = if open {
        end.saturating_sub(1)
    } else {
        fold_hint_row(lines, start, end).unwrap_or(start)
    };
    ParagraphSpan {
        key,
        start,
        end,
        expanded: open,
        control,
    }
}

/// 查找折叠段里的「N lines hidden」提示行。
///
/// 参数:
/// - `lines`: 文档行
/// - `start` / `end`: 段落行范围
///
/// 返回:
/// - 提示行号；段落没有折叠提示时为 None
fn fold_hint_row(lines: &[AnsiLine], start: usize, end: usize) -> Option<usize> {
    (start..end).find(|row| lines[*row].as_str().contains(FOLD_MARKER))
}

/// 展开段末尾的收起行。
///
/// 返回:
/// - 与折叠提示同样式的「收起」行
fn render_collapse_line() -> String {
    format!(
        "  \x1b[2m\x1b[36m{EXPANDED_MARKER} {}\x1b[0m",
        crate::render::terminal_text("Show less · click to collapse", "收起 · 点击折叠")
    )
}

/// 跳过段落开头的空行，返回首个有内容的行号。
///
/// 参数:
/// - `lines`: 文档行
/// - `start`: 段落起始行
///
/// 返回:
/// - 首个非空行；整段为空时返回起始行
fn first_content_row(lines: &[AnsiLine], start: usize) -> usize {
    lines[start..]
        .iter()
        .position(|line| {
            !crate::render::activity_animation::strip_ansi_for_test(line.as_str())
                .trim()
                .is_empty()
        })
        .map(|offset| start + offset)
        .unwrap_or(start)
}

/// 把用户消息压成单行摘要。
///
/// 参数:
/// - `text`: 用户原始输入
///
/// 返回:
/// - 折叠空白并截断后的摘要
pub(crate) fn summarize(text: &str) -> String {
    let collapsed = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if collapsed.chars().count() <= SUMMARY_CHARS {
        return collapsed;
    }
    let mut summary = collapsed.chars().take(SUMMARY_CHARS).collect::<String>();
    summary.push('…');
    summary
}

#[cfg(test)]
#[path = "fullscreen_view_tests.rs"]
mod tests;
