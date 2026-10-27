#[cfg(test)]
mod activity_alignment_tests;
mod appearance;
pub(in crate::render::transcript) mod cache;
#[cfg(test)]
mod compact_tests;
mod details;
mod file_summary;
mod group;
mod node;
mod older_steps;
#[cfg(test)]
mod older_steps_tests;
pub(in crate::render::transcript) mod recent_details;
#[cfg(test)]
mod recent_details_tests;
#[cfg(test)]
mod semantic_tool_tests;
mod standard_node;
#[cfg(test)]
mod standard_node_tests;
mod summary;
#[cfg(test)]
mod summary_tests;
#[cfg(test)]
mod tests;
mod tool_summary;

use super::cell::HistoryCell;
use super::fullscreen_parts::ParagraphKey;
use super::fullscreen_view::{first_content_row, summarize, FullscreenDocument, UserAnchor};
use super::line::AnsiLine;
use super::store::{TranscriptRenderOptions, TranscriptStore};
use crate::llm::ChatStreamKind;
use std::collections::HashSet;

pub(super) use group::is_activity;

impl TranscriptStore {
    /// 【终端】【线性执行链】把连续思考和工具组合为可折叠链，普通回答留在链外。
    /// 参数: `width` 为正文净宽，`options` 为可见性偏好，`expanded` 为手动折叠键；返回: 文档。
    pub(super) fn render_deepseek_document(
        &mut self,
        width: usize,
        options: &TranscriptRenderOptions,
        expanded: &HashSet<ParagraphKey>,
    ) -> FullscreenDocument {
        let width = width.max(1);
        let mut document = FullscreenDocument::default();
        let mut index = 0;
        let mut tail_grouped = false;
        // 1. 【终端】【线性执行链】分组不跨越用户消息、回答和系统消息，顺序与源记录一致
        while index < self.cells.len() {
            if is_activity(&self.cells[index]) {
                let end = (index..self.cells.len())
                    .find(|next| !is_activity(&self.cells[*next]))
                    .unwrap_or(self.cells.len());
                let include_tail = end == self.cells.len();
                append_group(
                    &mut document,
                    group::render(self, index..end, include_tail, width, options, expanded),
                );
                tail_grouped = include_tail;
                index = end;
                continue;
            }
            let start = document.lines.len();
            let key = ParagraphKey::whole(index);
            let expandable = matches!(&self.cells[index], HistoryCell::UserEcho(cell) if super::user_echo_cell::would_fold(cell));
            let open = expandable && expanded.contains(&key);
            let lines = if expandable {
                crate::render::omitted_line::with_fullscreen_hints(|| {
                    if open {
                        crate::render::render_expand::with_force_expand(|| {
                            self.cells[index].display_lines_framed(width, options, 0)
                        })
                    } else {
                        crate::render::render_expand::with_force_collapse(|| {
                            self.cells[index].display_lines_framed(width, options, 0)
                        })
                    }
                })
            } else {
                self.cache
                    .lines_for(index, &self.cells[index], width, options, 0)
            };
            append_lines(&mut document, lines);
            if open {
                document
                    .lines
                    .push(AnsiLine::new(super::fullscreen_view::render_collapse_line()));
            }
            if expandable && document.lines.len() > start {
                document.paragraphs.push(super::fullscreen_view::make_span(
                    &document.lines,
                    key,
                    start,
                    document.lines.len(),
                    open,
                ));
            }
            if let HistoryCell::UserEcho(cell) = &self.cells[index] {
                document.anchors.push(UserAnchor {
                    row: first_content_row(&document.lines, start),
                    summary: summarize(&cell.text),
                });
            }
            index += 1;
        }
        // 2. 【终端】【线性执行链】实时思考和参数接到最后一组，定稿后保留窗口内详情
        if !tail_grouped {
            let index = self.cells.len();
            append_group(
                &mut document,
                group::render(self, index..index, true, width, options, expanded),
            );
        }
        if self
            .live_tail
            .as_ref()
            .is_some_and(|tail| tail.kind == ChatStreamKind::Content)
        {
            let lines = self.render_live_tail_with_activity(width, options, usize::MAX, false);
            append_lines(&mut document, lines);
        }
        document
    }
}

/// 【终端】【执行链拼接】偏移分组内的点击范围，保留隐藏节点的展开键。
/// 参数: `document` 为完整文档，`group` 为相对坐标分组；返回: 无。
fn append_group(document: &mut FullscreenDocument, mut group: FullscreenDocument) {
    if group.lines.is_empty() {
        return;
    }
    if document
        .lines
        .last()
        .is_some_and(|line| !super::spacing::is_visual_blank(line))
    {
        document.lines.push(AnsiLine::new(String::new()));
    }
    let offset = document.lines.len();
    for span in &mut group.paragraphs {
        span.start += offset;
        span.end += offset;
        span.control += offset;
    }
    document.lines.extend(group.lines);
    document.paragraphs.extend(group.paragraphs);
    document.retained_keys.extend(group.retained_keys);
    for mut region in group.activities {
        region.start += offset;
        region.end += offset;
        document.activities.push(region);
    }
}

/// 【终端】【正文拼接】链外消息只保留一行前间距，不额外添加步骤竖线。
/// 参数: `document` 为文档，`lines` 为普通消息行；返回: 无。
fn append_lines(document: &mut FullscreenDocument, mut lines: Vec<AnsiLine>) {
    super::spacing::drop_duplicate_leading_blank(&mut lines, document.lines.last());
    if !lines.is_empty()
        && !document.lines.is_empty()
        && !super::spacing::is_visual_blank(&lines[0])
        && !super::spacing::is_visual_blank(document.lines.last().unwrap())
    {
        document.lines.push(AnsiLine::new(String::new()));
    }
    document.lines.extend(lines);
}
