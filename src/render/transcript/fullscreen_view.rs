//! 全屏会话视图的正文文档：按显式展开集合渲染完整 transcript。
//!
//! 与内联视图共用同一套 cell 渲染和区块空行规则；额外记录每个可折叠段落
//! 的行范围（供鼠标点击展开/收起）和每条用户消息的位置（供概览跳转与浮动标题）。

use super::fullscreen_parts::ParagraphKey;
use super::line::AnsiLine;
use super::store::{TranscriptRenderOptions, TranscriptStore, TranscriptView};
use crate::render::content_indent::{align_to_guide_column_with_width, CONTENT_LEFT_INDENT};
use crate::render::omitted_line::{rewrite_fold_hint_for_fullscreen, EXPANDED_MARKER, FOLD_MARKER};
use std::collections::HashSet;

/// 概览摘要最多保留的字符数。
const SUMMARY_CHARS: usize = 120;

/// 一个可点击展开/收起的段落在文档中的位置。
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ParagraphSpan {
    /// 段落键：流式尾部与定稿后使用相同的 cell 下标
    pub(crate) key: ParagraphKey,
    /// 首行（段落标题行）在文档中的行号
    pub(crate) start: usize,
    /// 末行之后的行号
    pub(crate) end: usize,
    /// 当前是否展开
    pub(crate) expanded: bool,
    /// 可点击的控制行：执行链节点为标题行；其余段落折叠时为「N lines hidden」提示，展开时为末尾的「收起」行
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

/// 【终端】【思考窗口】思考标题和完整正文的物理行范围，末行不含折叠控件。
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ReasoningRegion {
    pub(crate) key: ParagraphKey,
    pub(crate) start: usize,
    pub(crate) end: usize,
    pub(crate) live: bool,
}

/// 全屏视图一帧的完整正文。
#[derive(Clone, Debug, Default)]
pub(crate) struct FullscreenDocument {
    pub(crate) lines: Vec<AnsiLine>,
    pub(crate) paragraphs: Vec<ParagraphSpan>,
    pub(crate) anchors: Vec<UserAnchor>,
    /// 分组收起后仍保留子步骤的手动展开状态
    pub(crate) retained_keys: Vec<ParagraphKey>,
    /// 线性执行链的子视口范围
    pub(crate) activities: Vec<super::ActivityRegion>,
    pub(crate) reasonings: Vec<ReasoningRegion>,
}

impl FullscreenDocument {
    /// 查找包含指定行的可折叠段落。
    ///
    /// 参数:
    /// - `row`: 文档行号
    ///
    /// 返回:
    /// - 命中的段落
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
    /// - 控制行命中的段落；展开正文的点击范围由 paragraph_at 单独判断
    #[cfg(test)]
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
        let mut activities = document.activities.iter().peekable();
        for (row, line) in document.lines.iter_mut().enumerate() {
            let rewritten = rewrite_fold_hint_for_fullscreen(line.as_str());
            let source = rewritten.as_deref().unwrap_or(line.as_str());
            while activities.peek().is_some_and(|region| row >= region.end) {
                activities.next();
            }
            let in_activity = activities.peek().is_some_and(|region| row >= region.start);
            *line = AnsiLine::new(if in_activity {
                source.to_string()
            } else {
                align_to_guide_column_with_width(source, padding)
            });
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
        crate::render::tui_presentation::with_locale(|| {
            if let TranscriptView::Subagent { id, label } = self.view.clone() {
                return self.render_subagent_document(&id, &label, width, options, expanded);
            }
            self.render_activity_document(width, options, expanded)
        })
    }
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
pub(super) fn make_span(
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
pub(super) fn render_collapse_line() -> String {
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
pub(super) fn first_content_row(lines: &[AnsiLine], start: usize) -> usize {
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
